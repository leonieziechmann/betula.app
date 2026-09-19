package web

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leonieziechmann/btu-scraper/internal/model"
	"github.com/leonieziechmann/btu-scraper/internal/storage"
)

func setupTestServer(t *testing.T) (*Server, *storage.Storage) {
	tmpDir := t.TempDir()
	dbPath := filepath.Join(tmpDir, "test.db")

	store, err := storage.NewStorage(dbPath)
	if err != nil {
		t.Fatalf("failed to create test storage: %v", err)
	}

	// Insert test study program
	testProg := &model.OfficialStudyProgram{
		ID:          "stg_test_inf",
		ProgramName: "Informatik",
		Degree:      "Bachelor (universitär)",
		POVersion:   "2024",
	}
	if err := store.UpsertOfficialProgram(testProg); err != nil {
		t.Fatalf("failed to insert test program: %v", err)
	}

	// Insert test modules
	m1 := &model.ModuleDetail{
		ID:                     "11101",
		Code:                   "11101",
		TitleDE:                "Lineare Algebra I",
		Credits:                6.0,
		Turnus:                 "jedes Wintersemester",
		PrerequisitesMandatory: "keine",
	}
	m2 := &model.ModuleDetail{
		ID:                     "11102",
		Code:                   "11102",
		TitleDE:                "Lineare Algebra II",
		Credits:                6.0,
		Turnus:                 "jedes Sommersemester",
		PrerequisitesMandatory: "Kenntnis von Modul 11101",
	}
	_ = store.UpsertModuleDetail(m1)
	_ = store.UpsertModuleDetail(m2)
	_ = store.LinkModuleToStudyProgram(m1.ID, testProg.ID, testProg.ProgramName, testProg.Degree, testProg.POVersion)
	_ = store.LinkModuleToStudyProgram(m2.ID, testProg.ID, testProg.ProgramName, testProg.Degree, testProg.POVersion)

	server, err := NewServer(store, nil)
	if err != nil {
		t.Fatalf("failed to create server: %v", err)
	}

	return server, store
}

func TestIndexPage(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "BTU") || !stringsContains(body, "Smart Modulkatalog") {
		t.Errorf("expected HTML body to contain brand title, got: %s", body[:200])
	}
	if !stringsContains(body, "Informatik") {
		t.Errorf("expected HTML body to contain test study program Informatik")
	}
}

func TestModulesEndpoint_Prerequisites(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	// 1. Without having completed 11101: 11102 should report missing prerequisites
	req := httptest.NewRequest(http.MethodGet, "/modules?turnus=all", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "Lineare Algebra I") || !stringsContains(body, "Lineare Algebra II") {
		t.Errorf("expected modules to be listed")
	}
	if !stringsContains(body, "prereq-missing") || !stringsContains(body, "11101") {
		t.Errorf("expected prerequisite missing badge for module 11102, got body: %s", body)
	}

	// 2. With 11101 completed: 11102 should report prerequisites met
	req2 := httptest.NewRequest(http.MethodGet, "/modules?turnus=all&completed=11101", nil)
	rec2 := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec2, req2)

	if rec2.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec2.Code)
	}

	body2 := rec2.Body.String()
	if !stringsContains(body2, "Voraussetzungen erfüllt") {
		t.Errorf("expected prerequisites met badge for module 11102 when 11101 is completed")
	}
}

func TestModuleDetail(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	// 1. HTMX / In-page fetch should return the module detail HTML fragment
	req := httptest.NewRequest(http.MethodGet, "/modules/11101", nil)
	req.Header.Set("HX-Request", "true")
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "Lineare Algebra I") {
		t.Errorf("expected module detail to contain module title")
	}
	if !stringsContains(body, "module-detail-page") {
		t.Errorf("expected HTML to contain module-detail-page root container")
	}
	if !stringsContains(body, "Zurück zum Katalog") {
		t.Errorf("expected HTML to contain back to catalog button")
	}
	if !stringsContains(body, "Leistungspunkte") {
		t.Errorf("expected HTML to contain KPI grid")
	}

	// 2. Direct browser navigation with Sec-Fetch-Dest: document should redirect to /?module=11101
	browserReq := httptest.NewRequest(http.MethodGet, "/modules/11101", nil)
	browserReq.Header.Set("Sec-Fetch-Dest", "document")
	browserRec := httptest.NewRecorder()

	srv.mux.ServeHTTP(browserRec, browserReq)

	if browserRec.Code != http.StatusTemporaryRedirect {
		t.Fatalf("expected status 307 for direct browser navigation, got %d", browserRec.Code)
	}
	loc := browserRec.Header().Get("Location")
	if loc != "/?module=11101" {
		t.Errorf("expected redirect to /?module=11101, got %s", loc)
	}
}

func TestStatsEndpoint(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/api/stats", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "total_modules") {
		t.Errorf("expected JSON to contain total_modules, got %s", body)
	}
}

func TestStatsHTMLPage(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/stats", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "Telemetrie Dashboard") || !stringsContains(body, "DSGVO") {
		t.Errorf("expected HTML to contain dashboard title and DSGVO notice")
	}
}

func TestTrackingEndpoint(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	payload := `{"type":"module_click","target_id":"11101"}`
	req := httptest.NewRequest(http.MethodPost, "/api/track", strings.NewReader(payload))
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusNoContent {
		t.Fatalf("expected status 204 No Content, got %d", rec.Code)
	}
}

func TestLogsEndpoint(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/api/logs", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}
}

func TestIndexPage_QueryParams(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/?q=Algebra&turnus=wise_even", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, `value="Algebra"`) {
		t.Errorf("expected search input to have value=\"Algebra\", got: %s", body)
	}
	if !stringsContains(body, "nav-btn-share") {
		t.Errorf("expected HTML body to contain share button nav-btn-share")
	}
}

func TestModulesEndpoint_DirectBrowserRedirect(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/modules?q=Algebra&turnus=wise_even", nil)
	req.Header.Set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusTemporaryRedirect {
		t.Fatalf("expected status 307 Temporary Redirect for browser direct visit, got %d", rec.Code)
	}

	loc := rec.Header().Get("Location")
	if loc != "/?q=Algebra&turnus=wise_even" {
		t.Errorf("expected redirect location /?q=Algebra&turnus=wise_even, got %s", loc)
	}
}

func TestModulesEndpoint_ParamAliases(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	// Use program alias and prereqs_met alias with HTMX request header
	req := httptest.NewRequest(http.MethodGet, "/modules?program=stg_test_inf&prereqs_met=true&completed=11101", nil)
	req.Header.Set("HX-Request", "true")
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "Lineare Algebra II") {
		t.Errorf("expected Lineare Algebra II to be returned when prereqs_met alias is used with 11101 completed")
	}
}

func stringsContains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || (len(s) > 0 && len(substr) > 0 && (s != "" && containsHelper(s, substr))))
}

func containsHelper(s, substr string) bool {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
