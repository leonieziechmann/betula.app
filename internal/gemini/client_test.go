package gemini

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestExtractCurriculumFromPDF_Success(t *testing.T) {
	tmpDir := t.TempDir()
	pdfFile := filepath.Join(tmpDir, "sample_po.pdf")
	_ = os.WriteFile(pdfFile, []byte("%PDF-1.4 dummy"), 0644)

	mockResponse := geminiResponse{
		Candidates: []geminiCandidate{
			{
				Content: struct {
					Parts []struct {
						Text string `json:"text"`
					} `json:"parts"`
				}{
					Parts: []struct {
						Text string `json:"text"`
					}{
						{
							Text: `{
								"program_name": "Informatik",
								"degree": "Bachelor of Science",
								"po_version": "2024",
								"modules": [
									{
										"source_cell": "a",
                                        "module_code": "12104",
										"module_name": "Entwicklung von Softwaresystemen",
										"recommended_semester": 1,
										"credits": 8,
										"module_type": "Pflicht"
									},
									{
										"source_cell": "b",
                                        "module_code": "12101",
										"module_name": "Algorithmieren und Programmieren",
										"recommended_semester": 2,
										"credits": 10,
										"module_type": "Pflicht"
									}
								]
							}`,
						},
					},
				},
				FinishReason: "STOP",
			},
		},
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("key") != "" || r.Header.Get("x-goog-api-key") != "test-key" {
			t.Error("key must be in header, not URL")
		}
		if r.Method != "POST" {
			t.Errorf("expected POST method, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(mockResponse)
	}))
	defer server.Close()

	client := NewClient("test-key", "gemini-3.5-flash-lite")
	client.SetBaseURL(server.URL)
	client.layoutLoader = func(context.Context, string) (*PDFLayout, error) {
		return &PDFLayout{Cells: []SourceCell{
			{ID: "a", Row: "Entwicklung von Softwaresystemen", Semesters: []int{1}, Min: 8, Max: 8},
			{ID: "b", Row: "Algorithmieren und Programmieren", Semesters: []int{2}, Min: 10, Max: 10},
		}}, nil
	}

	res, err := client.ExtractCurriculumFromPDF(context.Background(), pdfFile, "Informatik")
	if err != nil {
		t.Fatalf("ExtractCurriculumFromPDF failed: %v", err)
	}

	if res.ProgramName != "Informatik" {
		t.Errorf("expected program name 'Informatik', got: %s", res.ProgramName)
	}
	if len(res.Modules) != 2 {
		t.Fatalf("expected 2 modules, got %d", len(res.Modules))
	}
	if res.Modules[0].ModuleCode != "" || res.Modules[0].RecommendedSemester != 1 {
		t.Errorf("unexpected module 0: %+v", res.Modules[0])
	}
}

func TestExtractCurriculumFromPDF_MissingKey(t *testing.T) {
	client := NewClient("", "gemini-3.5-flash-lite")
	_, err := client.ExtractCurriculumFromPDF(context.Background(), "some.pdf", "Informatik")
	if err == nil {
		t.Fatalf("expected error for missing key, got nil")
	}
}

func TestIncompleteResponsesAreNeverAccepted(t *testing.T) {
	for _, reason := range []string{"MAX_TOKENS", "SAFETY", ""} {
		t.Run(reason, func(t *testing.T) {
			pdf := filepath.Join(t.TempDir(), "test.pdf")
			if err := os.WriteFile(pdf, []byte("%PDF"), 0644); err != nil {
				t.Fatal(err)
			}
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				_ = json.NewEncoder(w).Encode(map[string]any{"candidates": []any{map[string]any{"finishReason": reason, "content": map[string]any{"parts": []any{map[string]string{"text": `{"program_name":"Test","modules":[]}`}}}}}})
			}))
			defer server.Close()
			client := NewClient("secret", "test")
			client.SetBaseURL(server.URL)
			client.layoutLoader = func(context.Context, string) (*PDFLayout, error) { return &PDFLayout{}, nil }
			if _, err := client.ExtractCurriculumFromPDF(context.Background(), pdf, "Test"); err == nil {
				t.Fatal("incomplete response accepted")
			}
		})
	}
}
