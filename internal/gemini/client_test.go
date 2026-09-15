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
										"module_code": "12104",
										"module_name": "Entwicklung von Softwaresystemen",
										"recommended_semester": 1,
										"credits": 8,
										"module_type": "Pflicht"
									},
									{
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
		if r.Method != "POST" {
			t.Errorf("expected POST method, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(mockResponse)
	}))
	defer server.Close()

	client := NewClient("test-key", "gemini-3.5-flash-lite")
	client.SetBaseURL(server.URL)

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
	if res.Modules[0].ModuleCode != "12104" || res.Modules[0].RecommendedSemester != 1 {
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
