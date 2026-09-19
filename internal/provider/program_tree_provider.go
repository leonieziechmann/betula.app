package provider

import (
	"bytes"
	"context"
	"crypto/sha1"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/cache"
	"github.com/leonieziechmann/btu-scraper/internal/model"
	"github.com/leonieziechmann/btu-scraper/internal/parser"
	"github.com/leonieziechmann/btu-scraper/internal/storage"
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

// DownloadDir returns the directory path where PDF statutes are saved.
func (p *BTUProgramTreeProvider) DownloadDir() string {
	return p.downloadDir
}

// SetDownloadDir configures the directory path where PDF statutes are saved.
func (p *BTUProgramTreeProvider) SetDownloadDir(dir string) {
	if dir != "" {
		p.downloadDir = dir
	}
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
					p.processDocumentDownload(ctx, &docs[i], prog.Name, forceRefresh)
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

// DownloadStats holds counts and statistics from a document download pass.
type DownloadStats struct {
	TotalPrograms  int
	TotalDocuments int
	UniqueURLs     int
	Downloaded     int
	SkippedCached  int
	Errors         int
}

// DownloadProgramDocuments downloads all regulation documents for the given programs.
// It deduplicates document downloads across programs, checks for existing cached files on disk,
// downloads missing files politely, updates document metadata (LocalPath and DownloadStatus),
// persists updates to storage if available, and returns the updated programs and stats.
func (p *BTUProgramTreeProvider) DownloadProgramDocuments(
	ctx context.Context,
	programs []model.OfficialStudyProgram,
	forceRefresh bool,
	workers int,
	delayMs int,
) ([]model.OfficialStudyProgram, DownloadStats, error) {
	if workers <= 0 {
		workers = 2
	}
	if delayMs < 0 {
		delayMs = 200
	}

	stats := DownloadStats{
		TotalPrograms: len(programs),
	}

	type docJob struct {
		url         string
		programName string
		doc         model.ProgramRegulationDocument
	}

	// 1. Collect all unique document URLs across programs
	uniqueMap := make(map[string]*docJob)
	var urlOrder []string

	for _, prog := range programs {
		for _, doc := range prog.Documents {
			stats.TotalDocuments++
			if doc.URL == "" {
				continue
			}
			if _, exists := uniqueMap[doc.URL]; !exists {
				uniqueMap[doc.URL] = &docJob{
					url:         doc.URL,
					programName: prog.ProgramName,
					doc:         doc,
				}
				urlOrder = append(urlOrder, doc.URL)
			}
		}
	}

	stats.UniqueURLs = len(urlOrder)
	if stats.UniqueURLs == 0 {
		return programs, stats, nil
	}

	// 2. Process each unique URL through a worker pool
	jobs := make(chan *docJob, len(urlOrder))
	for _, u := range urlOrder {
		jobs <- uniqueMap[u]
	}
	close(jobs)

	var mu sync.Mutex
	var completed uint64
	var wg sync.WaitGroup

	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				select {
				case <-ctx.Done():
					return
				case job, ok := <-jobs:
					if !ok {
						return
					}

					wasCached, err := p.downloadSingleDocument(ctx, &job.doc, job.programName, forceRefresh)

					mu.Lock()
					if err == nil && job.doc.DownloadStatus == "downloaded" {
						if wasCached {
							stats.SkippedCached++
						} else {
							stats.Downloaded++
						}
					} else {
						stats.Errors++
					}
					mu.Unlock()

					curr := atomic.AddUint64(&completed, 1)
					if curr%5 == 0 || curr == uint64(stats.UniqueURLs) {
						fmt.Printf("[Download] %d/%d (%.1f%%) documents processed\n",
							curr, stats.UniqueURLs, float64(curr)/float64(stats.UniqueURLs)*100)
					}

					if !wasCached && delayMs > 0 {
						time.Sleep(time.Duration(delayMs) * time.Millisecond)
					}
				}
			}
		}()
	}

	wg.Wait()

	// 3. Propagate updated status and local path back to all study programs
	for i := range programs {
		for j := range programs[i].Documents {
			if res, ok := uniqueMap[programs[i].Documents[j].URL]; ok {
				programs[i].Documents[j].LocalPath = res.doc.LocalPath
				programs[i].Documents[j].DownloadStatus = res.doc.DownloadStatus
			}
		}
	}

	// 4. Persist updated programs to storage
	if p.storage != nil {
		_ = p.storage.UpsertOfficialPrograms(programs)
	}

	return programs, stats, nil
}

// processDocumentDownload attempts to download a statute or amendment for a single document.
func (p *BTUProgramTreeProvider) processDocumentDownload(ctx context.Context, doc *model.ProgramRegulationDocument, programName string, forceRefresh bool) {
	_, _ = p.downloadSingleDocument(ctx, doc, programName, forceRefresh)
}

// downloadSingleDocument downloads or validates an existing PDF document.
// Returns wasCached=true if an existing valid file was found on disk and reused.
func (p *BTUProgramTreeProvider) downloadSingleDocument(
	ctx context.Context,
	doc *model.ProgramRegulationDocument,
	programName string,
	forceRefresh bool,
) (bool, error) {
	if doc.URL == "" {
		return false, fmt.Errorf("empty document URL")
	}

	progSlug := cleanSlug(programName, "program")
	dir := filepath.Join(p.downloadDir, progSlug)
	if err := os.MkdirAll(dir, 0755); err != nil {
		doc.DownloadStatus = "error"
		return false, fmt.Errorf("failed to create directory %s: %w", dir, err)
	}

	fileName := getDocumentFileName(doc.URL)
	targetPath := filepath.Join(dir, fileName)

	// Check if already downloaded on disk and valid
	if !forceRefresh {
		if fi, err := os.Stat(targetPath); err == nil && fi.Size() > 0 {
			doc.LocalPath = targetPath
			doc.DownloadStatus = "downloaded"
			return true, nil
		}
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, doc.URL, nil)
	if err != nil {
		doc.DownloadStatus = "error"
		return false, fmt.Errorf("failed to create request: %w", err)
	}
	// Use polite bot user-agent; desktop browser user-agents trigger OPUS 4 Cloudflare JS challenges
	req.Header.Set("User-Agent", "BTUScraper/1.0 (+https://www.b-tu.de; Academic Research)")
	req.Header.Set("Accept", "application/pdf,application/octet-stream,*/*")

	resp, err := p.client.Do(req)
	if err != nil {
		doc.DownloadStatus = "error"
		return false, fmt.Errorf("download request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		doc.DownloadStatus = "error"
		return false, fmt.Errorf("unexpected HTTP status %d", resp.StatusCode)
	}

	contentType := strings.ToLower(resp.Header.Get("Content-Type"))

	// Read initial header bytes (up to 4KB) to inspect content type and magic bytes
	preview := make([]byte, 4096)
	n, _ := io.ReadFull(resp.Body, preview)
	previewStr := strings.ToLower(string(preview[:n]))

	// Check for bot challenge pages
	if strings.Contains(contentType, "text/html") ||
		strings.Contains(previewStr, "not a bot") ||
		strings.Contains(previewStr, "cloudflare") ||
		strings.Contains(previewStr, "security check") ||
		strings.Contains(previewStr, "challenge-platform") {
		doc.DownloadStatus = "blocked_bot_checker"
		return false, fmt.Errorf("blocked by bot protection on %s", doc.URL)
	}

	// Verify it is genuinely a PDF
	if strings.Contains(contentType, "application/pdf") ||
		strings.Contains(contentType, "application/octet-stream") ||
		strings.HasPrefix(string(preview[:n]), "%PDF-") {

		tmpPath := targetPath + ".tmp"
		outFile, err := os.Create(tmpPath)
		if err != nil {
			doc.DownloadStatus = "error"
			return false, fmt.Errorf("failed to create temp file %s: %w", tmpPath, err)
		}

		if n > 0 {
			if _, err := outFile.Write(preview[:n]); err != nil {
				outFile.Close()
				_ = os.Remove(tmpPath)
				doc.DownloadStatus = "error"
				return false, fmt.Errorf("failed to write header bytes: %w", err)
			}
		}

		if _, err := io.Copy(outFile, resp.Body); err != nil {
			outFile.Close()
			_ = os.Remove(tmpPath)
			doc.DownloadStatus = "error"
			return false, fmt.Errorf("failed to save PDF body: %w", err)
		}
		outFile.Close()

		_ = os.Remove(targetPath)
		if err := os.Rename(tmpPath, targetPath); err != nil {
			doc.DownloadStatus = "error"
			return false, fmt.Errorf("failed to finalize file %s: %w", targetPath, err)
		}

		doc.LocalPath = targetPath
		doc.DownloadStatus = "downloaded"
		return false, nil
	}

	doc.DownloadStatus = "error"
	return false, fmt.Errorf("response was not a recognized PDF: %s", contentType)
}

// getDocumentFileName generates a clean, safe, and unique filename for a document URL.
func getDocumentFileName(rawURL string) string {
	parsed, err := url.Parse(rawURL)
	if err != nil {
		base := filepath.Base(rawURL)
		if !strings.HasSuffix(strings.ToLower(base), ".pdf") {
			base += ".pdf"
		}
		return sanitizeFileName(cleanSlug(base, "document.pdf"))
	}

	parts := strings.Split(strings.Trim(parsed.Path, "/"), "/")
	fileName := "document.pdf"
	docID := ""
	if len(parts) > 0 {
		fileName = parts[len(parts)-1]
	}
	if len(parts) >= 2 {
		for i := len(parts) - 2; i >= 0 && i >= len(parts)-4; i-- {
			if _, err := strconv.Atoi(parts[i]); err == nil {
				docID = parts[i]
				break
			}
		}
	}

	// Strip any query parameters or hash from filename
	if idx := strings.IndexAny(fileName, "?#"); idx != -1 {
		fileName = fileName[:idx]
	}
	if !strings.HasSuffix(strings.ToLower(fileName), ".pdf") {
		fileName += ".pdf"
	}
	if docID != "" && !strings.HasPrefix(fileName, docID+"_") {
		fileName = docID + "_" + fileName
	}

	return sanitizeFileName(fileName)
}

func sanitizeFileName(s string) string {
	s = strings.TrimSpace(s)
	invalid := []string{":", "*", "?", "\"", "<", ">", "|", "\\", "/"}
	for _, inv := range invalid {
		s = strings.ReplaceAll(s, inv, "_")
	}
	return s
}

func cleanSlug(s, fallback string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		s = fallback
	}
	invalid := []string{" ", "/", "\\", ":", "|", "*", "?", "\"", "<", ">"}
	for _, inv := range invalid {
		s = strings.ReplaceAll(s, inv, "_")
	}
	return s
}

// TraverseQISCurriculum traverses the nested QIS tree for a specific PO and extracts all categorized module assignments.
func (p *BTUProgramTreeProvider) TraverseQISCurriculum(
	ctx context.Context,
	poURL string,
	prog model.OfficialStudyProgram,
	delayMs int,
) ([]model.CurriculumModule, error) {
	if delayMs <= 0 {
		delayMs = 300
	}

	var results []model.CurriculumModule
	visited := make(map[string]bool)
	seenModules := make(map[string]bool)

	var walk func(currentURL string, path []string)
	walk = func(currentURL string, path []string) {
		select {
		case <-ctx.Done():
			return
		default:
		}

		if visited[currentURL] {
			return
		}
		visited[currentURL] = true

		if delayMs > 0 {
			time.Sleep(time.Duration(delayMs) * time.Millisecond)
		}

		htmlBytes, err := p.fetchHTML(ctx, currentURL, false)
		if err != nil {
			return
		}

		nodes, err := p.parser.ParsePOBranchNodes(bytes.NewReader(htmlBytes), "https://www.b-tu.de")
		if err != nil {
			return
		}

		for _, node := range nodes {
			if node.IsModule {
				studySection, subjectArea, moduleType, specialization := parser.AnalyzeQISPath(path)
				modCode := node.ModuleID
				if modCode == "" && len(node.Text) >= 5 {
					if m := regexp.MustCompile(`^(\d{5})`).FindStringSubmatch(node.Text); len(m) > 1 {
						modCode = m[1]
					}
				}

				key := fmt.Sprintf("%s_%s_%s_%s", modCode, node.Title, studySection, subjectArea)
				if !seenModules[key] {
					seenModules[key] = true
					results = append(results, model.CurriculumModule{
						ProgramID:      prog.ID,
						ProgramName:    prog.ProgramName,
						Degree:         prog.Degree,
						POVersion:      prog.POVersion,
						ModuleID:       modCode,
						ModuleCode:     modCode,
						ModuleName:     node.Title,
						ModuleType:     moduleType,
						StudySection:   studySection,
						SubjectArea:    subjectArea,
						Specialization: specialization,
						SourceFile:     "qis_tree",
						ExtractedAt:    time.Now(),
					})
				}
			} else {
				// Category branch (e.g. Grundstudium, Fachstudium, Praktische Informatik, etc.)
				if !containsString(path, node.Text) &&
					!strings.HasPrefix(node.Text, "PO-Version") &&
					!strings.HasPrefix(node.Text, "Studiengang") &&
					!strings.HasPrefix(node.Text, "Module für Abschluss") {
					newPath := append(append([]string{}, path...), node.Text)
					walk(node.URL, newPath)
				}
			}
		}
	}

	walk(poURL, nil)
	return results, nil
}

func containsString(slice []string, val string) bool {
	for _, s := range slice {
		if s == val {
			return true
		}
	}
	return false
}

