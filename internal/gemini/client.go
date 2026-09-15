package gemini

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"time"
)

const (
	DefaultModel   = "gemini-3.5-flash-lite"
	DefaultBaseURL = "https://generativelanguage.googleapis.com/v1beta/models"
)

// ExtractedModule represents a single course or requirement extracted from a study regulation PDF.
type ExtractedModule struct {
	ModuleCode             string  `json:"module_code,omitempty"`
	ModuleName             string  `json:"module_name"`
	ModuleNameEN           string  `json:"module_name_en,omitempty"`
	RecommendedSemester    int     `json:"recommended_semester,omitempty"`
	RecommendedSemesterRaw string  `json:"recommended_semester_raw,omitempty"`
	Credits                float64 `json:"credits,omitempty"`
	ModuleType             string  `json:"module_type"` // "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
	Specialization         string  `json:"specialization,omitempty"`
	SWS                    string  `json:"sws,omitempty"`
	ExamType               string  `json:"exam_type,omitempty"`
	Graded                 string  `json:"graded,omitempty"`
	Prerequisites          string  `json:"prerequisites,omitempty"`
	Remarks                string  `json:"remarks,omitempty"`
}

// CurriculumExtractionResult represents the structured extraction result from a study regulation PDF.
type CurriculumExtractionResult struct {
	ProgramName             string            `json:"program_name"`
	Degree                  string            `json:"degree"`
	POVersion               string            `json:"po_version"`
	StandardPeriodSemesters int               `json:"standard_period_semesters,omitempty"`
	TotalCredits            float64           `json:"total_credits,omitempty"`
	Modules                 []ExtractedModule `json:"modules"`
}

// Client interacts with the Google Gemini API.
type Client struct {
	apiKey     string
	model      string
	baseURL    string
	httpClient *http.Client
}

// NewClient creates a new Gemini client.
func NewClient(apiKey, model string) *Client {
	if model == "" {
		model = DefaultModel
	}
	return &Client{
		apiKey:  apiKey,
		model:   model,
		baseURL: DefaultBaseURL,
		httpClient: &http.Client{
			Timeout: 120 * time.Second,
		},
	}
}

// SetBaseURL overrides the API base URL (useful for mock testing).
func (c *Client) SetBaseURL(url string) {
	c.baseURL = url
}

type geminiPart struct {
	Text       string            `json:"text,omitempty"`
	InlineData *geminiInlineData `json:"inlineData,omitempty"`
}

type geminiInlineData struct {
	MimeType string `json:"mimeType"`
	Data     string `json:"data"`
}

type geminiContent struct {
	Role  string       `json:"role,omitempty"`
	Parts []geminiPart `json:"parts"`
}

type geminiRequest struct {
	Contents         []geminiContent  `json:"contents"`
	GenerationConfig *geminiGenConfig `json:"generationConfig,omitempty"`
}

type geminiGenConfig struct {
	ResponseMimeType string          `json:"responseMimeType,omitempty"`
	ResponseSchema   json.RawMessage `json:"responseSchema,omitempty"`
	Temperature      float64         `json:"temperature,omitempty"`
}

type geminiCandidate struct {
	Content struct {
		Parts []struct {
			Text string `json:"text"`
		} `json:"parts"`
	} `json:"content"`
	FinishReason string `json:"finishReason"`
}

type geminiResponse struct {
	Candidates []geminiCandidate `json:"candidates"`
	Error      *struct {
		Code    int    `json:"code"`
		Message string `json:"message"`
		Status  string `json:"status"`
	} `json:"error,omitempty"`
}

const curriculumSchema = `{
	"type": "OBJECT",
	"properties": {
		"program_name": { "type": "STRING" },
		"degree": { "type": "STRING" },
		"po_version": { "type": "STRING" },
		"standard_period_semesters": { "type": "INTEGER" },
		"total_credits": { "type": "NUMBER" },
		"modules": {
			"type": "ARRAY",
			"items": {
				"type": "OBJECT",
				"properties": {
					"module_code": { "type": "STRING" },
					"module_name": { "type": "STRING" },
					"module_name_en": { "type": "STRING" },
					"recommended_semester": { "type": "INTEGER" },
					"recommended_semester_raw": { "type": "STRING" },
					"credits": { "type": "NUMBER" },
					"module_type": { "type": "STRING", "enum": ["Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"] },
					"specialization": { "type": "STRING" },
					"sws": { "type": "STRING" },
					"exam_type": { "type": "STRING" },
					"graded": { "type": "STRING" },
					"prerequisites": { "type": "STRING" },
					"remarks": { "type": "STRING" }
				},
				"required": ["module_name", "module_type"]
			}
		}
	},
	"required": ["program_name", "modules"]
}`

// ExtractCurriculumFromPDF sends a regulation PDF to Gemini and parses the structured curriculum.
func (c *Client) ExtractCurriculumFromPDF(ctx context.Context, pdfPath string, programHint string) (*CurriculumExtractionResult, error) {
	if c.apiKey == "" {
		return nil, fmt.Errorf("Gemini API key is not configured. Set gemini.api_key in config.yaml or GEMINI_API_KEY environment variable")
	}

	pdfBytes, err := os.ReadFile(pdfPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read PDF file %s: %w", pdfPath, err)
	}

	b64Data := base64.StdEncoding.EncodeToString(pdfBytes)

	prompt := fmt.Sprintf(`Du bist ein Experte für deutsche Hochschulprüfungsordnungen der BTU Cottbus-Senftenberg.
Analysiere die beigefügte Studien- und Prüfungsordnung (bzw. Satzungsänderung) gründlich.
Hinweis zum Studiengang: "%s" (Dateiname: %s).

Extrahiere den kompletten Studienverlaufsplan / Regelstudienplan / die Modultabelle (Anlage zur Prüfungsordnung):
1. Ermittle den genauen Studiengang (program_name), Abschluss (degree) und die PO-Version / das Amtsblatt (po_version).
2. Extrahiere ALLE aufgeführten Module und Lehrveranstaltungen mit:
   - module_code: Modulnummer/Modulcode, falls in der Tabelle angegeben (oft 5-stellig wie z.B. 11101, 12104, oder Kürzel)
   - module_name: Offizieller Modulname auf Deutsch
   - module_name_en: Englischer Name falls vorhanden
   - recommended_semester: Empfohlenes Fachsemester als Zahl (1, 2, 3, 4, 5, 6...). Falls unklar, 0.
   - recommended_semester_raw: Semesterangabe im Original (z.B. "1.", "1.-2.", "3. oder 4.")
   - credits: Leistungspunkte / ECTS / LP als Dezimalzahl (z.B. 6.0)
   - module_type: Art des Moduls. Verwende genau einen dieser Werte: "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
   - specialization: Vertiefungsrichtung, Studienschwerpunkt oder Wahlpflichtkatalog, falls das Modul zu einem bestimmten Schwerpunkt gehört (z.B. "Cybersecurity", "Katalog Technische Informatik"). Leer lassen wenn für alle gültig (Pflichtbereich).
   - sws: Semesterwochenstunden falls vorhanden (z.B. "4 SWS", "2V+2Ü")
   - exam_type: Art der Prüfungsleistung (z.B. "Klausur", "Mündliche Prüfung", "Hausarbeit", "Projekt")
   - graded: "benotet" oder "unbenotet"
   - prerequisites: Teilnahmevoraussetzungen falls in der Tabelle angegeben
   - remarks: Besondere Hinweise oder Fußnoten

Gib ausschließlich valides JSON gemäß dem vorgegebenen Schema zurück.`, programHint, filepath.Base(pdfPath))

	reqPayload := geminiRequest{
		Contents: []geminiContent{
			{
				Parts: []geminiPart{
					{Text: prompt},
					{
						InlineData: &geminiInlineData{
							MimeType: "application/pdf",
							Data:     b64Data,
						},
					},
				},
			},
		},
		GenerationConfig: &geminiGenConfig{
			ResponseMimeType: "application/json",
			ResponseSchema:   json.RawMessage(curriculumSchema),
			Temperature:      0.1,
		},
	}

	bodyBytes, err := json.Marshal(reqPayload)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/%s:generateContent?key=%s", c.baseURL, c.model, c.apiKey)

	// Retry up to 3 times for transient 503 / 429 responses
	var respBody []byte
	var lastErr error

	for attempt := 1; attempt <= 3; attempt++ {
		httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(bodyBytes))
		if err != nil {
			return nil, fmt.Errorf("failed to create http request: %w", err)
		}
		httpReq.Header.Set("Content-Type", "application/json")

		resp, err := c.httpClient.Do(httpReq)
		if err != nil {
			lastErr = fmt.Errorf("gemini api request failed: %w", err)
			time.Sleep(time.Duration(attempt) * 2 * time.Second)
			continue
		}

		respBody, err = io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			lastErr = fmt.Errorf("failed to read response: %w", err)
			continue
		}

		if resp.StatusCode == http.StatusOK {
			lastErr = nil
			break
		}

		if resp.StatusCode == http.StatusServiceUnavailable || resp.StatusCode == http.StatusTooManyRequests {
			lastErr = fmt.Errorf("gemini api temporary error %d: %s", resp.StatusCode, string(respBody))
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(time.Duration(attempt) * 3 * time.Second):
			}
			continue
		}

		return nil, fmt.Errorf("gemini api returned status %d: %s", resp.StatusCode, string(respBody))
	}

	if lastErr != nil {
		return nil, lastErr
	}

	var geminiResp geminiResponse
	if err := json.Unmarshal(respBody, &geminiResp); err != nil {
		return nil, fmt.Errorf("failed to parse gemini response: %w", err)
	}

	if geminiResp.Error != nil {
		return nil, fmt.Errorf("gemini api error %d (%s): %s", geminiResp.Error.Code, geminiResp.Error.Status, geminiResp.Error.Message)
	}

	if len(geminiResp.Candidates) == 0 || len(geminiResp.Candidates[0].Content.Parts) == 0 {
		return nil, fmt.Errorf("gemini returned no content candidates")
	}

	jsonText := geminiResp.Candidates[0].Content.Parts[0].Text

	var result CurriculumExtractionResult
	if err := json.Unmarshal([]byte(jsonText), &result); err != nil {
		return nil, fmt.Errorf("failed to parse structured curriculum JSON (%w): %s", err, jsonText)
	}

	return &result, nil
}
