package web

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/storage"
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

func TestModuleModal(t *testing.T) {
	srv, store := setupTestServer(t)
	defer store.Close()

	req := httptest.NewRequest(http.MethodGet, "/modules/11101", nil)
	rec := httptest.NewRecorder()

	srv.mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d", rec.Code)
	}

	body := rec.Body.String()
	if !stringsContains(body, "Lineare Algebra I") {
		t.Errorf("expected modal to contain module title")
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
