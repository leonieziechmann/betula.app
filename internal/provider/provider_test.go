package provider

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"

	"github.com/jakob/btu-scraper/internal/model"
)

type mockProvider struct {
	name string
	desc string
}

func (m *mockProvider) Name() string        { return m.name }
func (m *mockProvider) Description() string { return m.desc }

func TestRegistry(t *testing.T) {
	reg := NewRegistry()

	p1 := &mockProvider{name: "prov1", desc: "Provider 1"}
	p2 := &mockProvider{name: "prov2", desc: "Provider 2"}

	if err := reg.Register(p1); err != nil {
		t.Fatalf("failed to register p1: %v", err)
	}
	if err := reg.Register(p2); err != nil {
		t.Fatalf("failed to register p2: %v", err)
	}

	// Duplicate registration error
	if err := reg.Register(p1); err == nil {
		t.Fatalf("expected error on duplicate register, got nil")
	}

	// Get
	retrieved, ok := reg.Get("prov1")
	if !ok || retrieved.Name() != "prov1" {
		t.Errorf("failed to retrieve prov1: ok=%v", ok)
	}

	// List
	list := reg.List()
	if len(list) != 2 {
		t.Errorf("expected 2 providers in list, got %d", len(list))
	}
}

func TestProvidersRegistered(t *testing.T) {
	reg := NewRegistry()
	cat := NewBTUModuleCatalogProvider(nil, nil, "", 0)
	detail := NewBTUModuleDetailProvider(nil, nil, "", 0)

	_ = reg.Register(cat)
	_ = reg.Register(detail)
	event := NewBTUEventProvider(nil, nil, 0)
	_ = reg.Register(event)
	fues := NewBTUFUESProvider(nil, nil, "", 0)
	_ = reg.Register(fues)
	programs := NewBTUProgramTreeProvider(nil, nil, "", 0, "")
	_ = reg.Register(programs)

	if _, ok := reg.Get(CatalogProviderName); !ok {
		t.Errorf("expected %s to be registered", CatalogProviderName)
	}
	if _, ok := reg.Get(DetailProviderName); !ok {
		t.Errorf("expected %s to be registered", DetailProviderName)
	}
	if _, ok := reg.Get(EventProviderName); !ok {
		t.Errorf("expected %s to be registered", EventProviderName)
	}
	if _, ok := reg.Get(FUESProviderName); !ok {
		t.Errorf("expected %s to be registered", FUESProviderName)
	}
	if _, ok := reg.Get(ProgramTreeProviderName); !ok {
		t.Errorf("expected %s to be registered", ProgramTreeProviderName)
	}
}

type dummyCatalogProvider struct {
	mockProvider
}

func (d *dummyCatalogProvider) ScrapeCatalog(ctx context.Context, forceRefresh bool) (int, error) {
	return 42, nil
}

func TestCatalogProviderInterface(t *testing.T) {
	var _ CatalogProvider = &dummyCatalogProvider{}
}

func TestGetDocumentFileName(t *testing.T) {
	cases := []struct {
		url      string
		expected string
	}{
		{
			url:      "https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf",
			expected: "6707_12_Informatik_B.Sc.pdf",
		},
		{
			url:      "http://opus.kobv.de/btu/volltexte/2007/360/pdf/14_eBusiness.pdf",
			expected: "360_14_eBusiness.pdf",
		},
		{
			url:      "https://example.com/statutes/AMbl-10_2026.pdf?v=2#page=1",
			expected: "AMbl-10_2026.pdf",
		},
		{
			url:      "https://example.com/test:invalid*name.pdf",
			expected: "test_invalid_name.pdf",
		},
	}

	for _, tc := range cases {
		got := getDocumentFileName(tc.url)
		if got != tc.expected {
			t.Errorf("getDocumentFileName(%q) = %q; expected %q", tc.url, got, tc.expected)
		}
	}
}

func TestDownloadSingleDocumentAndCaching(t *testing.T) {
	tmpDir := t.TempDir()

	pdfContent := []byte("%PDF-1.4\n%mock pdf content for testing\n%%EOF")
	callCount := 0

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		// Verify User-Agent
		ua := r.Header.Get("User-Agent")
		if !strings.Contains(ua, "BTUScraper") {
			t.Errorf("expected User-Agent to contain BTUScraper, got: %s", ua)
		}

		if strings.Contains(r.URL.Path, "challenge") {
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			w.Write([]byte("<!doctype html><html><head><title>Making sure you're not a bot!</title></head><body></body></html>"))
			return
		}

		w.Header().Set("Content-Type", "application/pdf")
		w.Write(pdfContent)
	}))
	defer server.Close()

	prov := NewBTUProgramTreeProvider(nil, nil, "", 0, tmpDir)

	// 1. Initial download
	doc := model.ProgramRegulationDocument{
		Title: "Test PO",
		URL:   server.URL + "/files/1001/test_po.pdf",
	}

	wasCached, err := prov.downloadSingleDocument(context.Background(), &doc, "Informatik", false)
	if err != nil {
		t.Fatalf("unexpected error downloading document: %v", err)
	}
	if wasCached {
		t.Errorf("expected wasCached=false on initial download, got true")
	}
	if doc.DownloadStatus != "downloaded" {
		t.Errorf("expected DownloadStatus='downloaded', got: %s", doc.DownloadStatus)
	}
	if doc.LocalPath == "" {
		t.Errorf("expected LocalPath to be set")
	}
	if _, err := os.Stat(doc.LocalPath); err != nil {
		t.Errorf("target file does not exist on disk: %v", err)
	}

	// 2. Second download with forceRefresh=false should use cache
	wasCached2, err := prov.downloadSingleDocument(context.Background(), &doc, "Informatik", false)
	if err != nil {
		t.Fatalf("unexpected error on second call: %v", err)
	}
	if !wasCached2 {
		t.Errorf("expected wasCached=true on cached check, got false")
	}
	if callCount != 1 {
		t.Errorf("expected server to be called only once, but got %d calls", callCount)
	}

	// 3. Challenge detection
	botDoc := model.ProgramRegulationDocument{
		Title: "Bot Challenge Doc",
		URL:   server.URL + "/challenge.pdf",
	}
	_, errBot := prov.downloadSingleDocument(context.Background(), &botDoc, "Informatik", false)
	if errBot == nil {
		t.Errorf("expected error for bot challenge, got nil")
	}
	if botDoc.DownloadStatus != "blocked_bot_checker" {
		t.Errorf("expected DownloadStatus='blocked_bot_checker', got: %s", botDoc.DownloadStatus)
	}
}

func TestDownloadProgramDocumentsBatch(t *testing.T) {
	tmpDir := t.TempDir()

	pdfContent := []byte("%PDF-1.4\n%shared statute\n%%EOF")
	serverCalls := 0

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		serverCalls++
		w.Header().Set("Content-Type", "application/pdf")
		w.Write(pdfContent)
	}))
	defer server.Close()

	prov := NewBTUProgramTreeProvider(nil, nil, "", 0, tmpDir)

	sharedURL := server.URL + "/files/5555/shared_statute.pdf"

	programs := []model.OfficialStudyProgram{
		{
			ID:          "prog1",
			ProgramName: "Informatik",
			Degree:      "Bachelor",
			Documents: []model.ProgramRegulationDocument{
				{Title: "Gemeinsame Ordnung", URL: sharedURL},
			},
		},
		{
			ID:          "prog2",
			ProgramName: "Informatik",
			Degree:      "Master",
			Documents: []model.ProgramRegulationDocument{
				{Title: "Gemeinsame Ordnung", URL: sharedURL},
			},
		},
	}

	updated, stats, err := prov.DownloadProgramDocuments(context.Background(), programs, false, 2, 0)
	if err != nil {
		t.Fatalf("DownloadProgramDocuments failed: %v", err)
	}

	if stats.UniqueURLs != 1 {
		t.Errorf("expected 1 unique URL, got %d", stats.UniqueURLs)
	}
	if stats.Downloaded != 1 {
		t.Errorf("expected 1 downloaded, got %d", stats.Downloaded)
	}
	if serverCalls != 1 {
		t.Errorf("expected server to be called exactly once due to deduplication, got %d", serverCalls)
	}

	// Verify both programs have the document marked as downloaded with the same local path
	if updated[0].Documents[0].DownloadStatus != "downloaded" {
		t.Errorf("prog1 doc not marked as downloaded")
	}
	if updated[1].Documents[0].DownloadStatus != "downloaded" {
		t.Errorf("prog2 doc not marked as downloaded")
	}
	if updated[0].Documents[0].LocalPath != updated[1].Documents[0].LocalPath {
		t.Errorf("expected identical LocalPath, got %q vs %q",
			updated[0].Documents[0].LocalPath, updated[1].Documents[0].LocalPath)
	}
}
