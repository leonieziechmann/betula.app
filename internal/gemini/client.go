package gemini

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	DefaultModel   = "gemini-3.5-flash-lite"
	DefaultBaseURL = "https://generativelanguage.googleapis.com/v1beta/models"
)

// ExtractedModule represents a single course or requirement extracted from a study regulation PDF.
type ExtractedModule struct {
	SourceCell             string  `json:"source_cell"`
	ModuleCode             string  `json:"module_code,omitempty"`
	ModuleName             string  `json:"module_name"`
	ModuleNameEN           string  `json:"module_name_en,omitempty"`
	RecommendedSemester    int     `json:"recommended_semester,omitempty"`
	RecommendedSemesterRaw string  `json:"recommended_semester_raw,omitempty"`
	SemesterSpan           string  `json:"semester_span,omitempty"` // e.g. "5-6", "3-4"
	StartSemester          int     `json:"start_semester,omitempty"`
	EndSemester            int     `json:"end_semester,omitempty"`
	Credits                float64 `json:"credits,omitempty"`
	MinCredits             float64 `json:"min_credits,omitempty"`   // e.g. 10.0 for "10-24"
	MaxCredits             float64 `json:"max_credits,omitempty"`   // e.g. 24.0 for "10-24"
	ModuleType             string  `json:"module_type"`             // "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
	StudySection           string  `json:"study_section,omitempty"` // "Grundstudium", "Fachstudium", "Vertiefungsstudium"
	SubjectArea            string  `json:"subject_area,omitempty"`  // "Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Nebenfach", etc.
	AreaRules              string  `json:"area_rules,omitempty"`    // e.g. "Im Nebenfach müssen alle Module aus demselben Bereich gewählt werden", "Mind. 12 LP"
	Specialization         string  `json:"specialization,omitempty"`
	SWS                    string  `json:"sws,omitempty"`
	ExamType               string  `json:"exam_type,omitempty"`
	Graded                 string  `json:"graded,omitempty"`
	Prerequisites          string  `json:"prerequisites,omitempty"`
	Remarks                string  `json:"remarks,omitempty"`
	// Where in the regulation the row stands. Bound from the source cell, never
	// proposed by the model.
	SourcePage      int    `json:"source_page,omitempty"`
	SourceTable     string `json:"source_table,omitempty"`
	SourcePlanLabel string `json:"source_plan_label,omitempty"`
}

// CurriculumExtractionResult represents the structured extraction result from a study regulation PDF.
type CurriculumExtractionResult struct {
	SourceSHA256            string            `json:"source_sha256"`
	StartTerm               string            `json:"start_term"`
	Layout                  *PDFLayout        `json:"layout,omitempty"`
	ProgramName             string            `json:"program_name"`
	Degree                  string            `json:"degree"`
	POVersion               string            `json:"po_version"`
	StandardPeriodSemesters int               `json:"standard_period_semesters,omitempty"`
	TotalCredits            float64           `json:"total_credits,omitempty"`
	Modules                 []ExtractedModule `json:"modules"`
	// Totals are the sums the plan prints over its own rows, bound to the rows
	// they count (plan_totals.go). They say what a plan with elective budgets
	// adds up to, which its rows alone cannot.
	Totals []PlanTotal `json:"totals,omitempty"`
}

// Client interacts with the Google Gemini API.
type Client struct {
	PlanPages    []int
	layoutLoader func(context.Context, string) (*PDFLayout, error)
	apiKey       string
	model        string
	baseURL      string
	httpClient   *http.Client
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

// SetModel overrides the model name.
func (c *Client) SetModel(model string) {
	c.model = model
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
	Temperature      float64         `json:"temperature"`
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
					"source_cell": { "type": "STRING" },
					"module_code": { "type": "STRING" },
					"module_name": { "type": "STRING" },
					"module_name_en": { "type": "STRING" },
					"recommended_semester": { "type": "INTEGER" },
					"recommended_semester_raw": { "type": "STRING" },
					"semester_span": { "type": "STRING" },
					"start_semester": { "type": "INTEGER" },
					"end_semester": { "type": "INTEGER" },
					"credits": { "type": "NUMBER" },
					"min_credits": { "type": "NUMBER" },
					"max_credits": { "type": "NUMBER" },
					"module_type": { "type": "STRING", "enum": ["Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"] },
					"study_section": { "type": "STRING" },
					"subject_area": { "type": "STRING" },
					"area_rules": { "type": "STRING" },
					"specialization": { "type": "STRING" },
					"sws": { "type": "STRING" },
					"exam_type": { "type": "STRING" },
					"graded": { "type": "STRING" },
					"prerequisites": { "type": "STRING" },
					"remarks": { "type": "STRING" }
				},
				"required": ["source_cell", "module_name", "module_type", "recommended_semester", "credits"]
			}
		}
	},
	"required": ["program_name", "modules"]
}`

// ExtractCurriculumFromPDF sends a regulation PDF to Gemini and parses the structured curriculum.
func (c *Client) ExtractCurriculumFromPDF(ctx context.Context, pdfPath string, programHint string) (*CurriculumExtractionResult, error) {
	if c.apiKey == "" {
		// No API access configured: the deterministic PDF reader is sufficient.
		return c.ExtractCurriculumOffline(ctx, pdfPath, programHint)
	}

	pdfBytes, err := os.ReadFile(pdfPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read PDF file %s: %w", pdfPath, err)
	}

	b64Data := base64.StdEncoding.EncodeToString(pdfBytes)

	var layout *PDFLayout
	if c.layoutLoader != nil {
		layout, err = c.layoutLoader(ctx, pdfPath)
	} else {
		layout, err = ReadPDFLayoutForProgram(ctx, pdfPath, c.PlanPages, programHint)
	}
	if err != nil {
		return nil, err
	}
	selectProgramMode(layout, programHint)
	layoutJSON, err := json.Marshal(layout.Cells)
	if err != nil {
		return nil, err
	}
	prompt := fmt.Sprintf(`Extrahiere den Regelstudienplan aus der PDF für %s (%s).
Die beigefügten Daten wurden anhand der PDF-Zellkoordinaten ausgelesen. Leere Zellen
bleiben leer, horizontale/vertikale Zellverbindungen sind bereits berücksichtigt.
Gib GENAU EINEN Moduleintrag pro Eintrag im Quellzellen-Array aus und übernimm dessen ID als
source_cell. module_name muss dem Originalnamen in "row" entsprechen (ohne Modulnummer).
Ergänze Modulnummer, Pflicht/Wahlpflicht, Fachbereich usw. aus den übrigen PDF-Anlagen.
Bei shared_rows=true handelt es sich um EINEN gemeinsamen Wahlpflicht-Slot für ALLE
angegebenen Bereiche, nicht um ein bestimmtes Modul oder mehrere Slots.
"totals" sind nur Kontrollsummen und dürfen NICHT als Module ausgegeben werden.
Semesterspalten stammen ausschließlich aus "semesters", niemals aus Reihenfolge,
Modulnummer, Prüfungsdatum, LP-Summenspalte oder dem Wunsch nach 30 LP.
Bei mehreren Semestern in einer Zelle ist recommended_semester=0: die Quelle legt
kein einzelnes Semester fest. Übernimm Start/Ende und semester_span.
Bei LP-Spannen setze credits=0, min_credits=min und max_credits=max. Keine Mittelwerte!
Mehrere Zellen derselben Modulzeile sind getrennte Einträge mit den jeweiligen LP.
Erfinde keine Module oder Werte. Nutze Originalnamen, keine Übersetzung.
Wenn der Studienplan nicht zum angefragten Studiengang/Abschluss passt, gib modules=[] aus.
JSON gemäß Schema. Quellzellen und Tabellen:
%s`, programHint, filepath.Base(pdfPath), layoutJSON)

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
			Temperature:      0,
		},
	}

	bodyBytes, err := json.Marshal(reqPayload)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/%s:generateContent", c.baseURL, c.model)

	// Retry up to 3 times for transient 503 / 429 responses
	var respBody []byte
	var lastErr error

	for attempt := 1; attempt <= 3; attempt++ {
		httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(bodyBytes))
		if err != nil {
			return nil, fmt.Errorf("failed to create http request: %w", err)
		}
		httpReq.Header.Set("Content-Type", "application/json")
		httpReq.Header.Set("x-goog-api-key", c.apiKey)

		resp, err := c.httpClient.Do(httpReq)
		if err != nil {
			lastErr = fmt.Errorf("gemini api request failed: %w", err)
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(time.Duration(attempt) * 2 * time.Second):
			}
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
			lastErr = fmt.Errorf("gemini api temporary error %d: %s", resp.StatusCode, strings.ReplaceAll(string(respBody), c.apiKey, "[REDACTED]"))
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(time.Duration(attempt) * 3 * time.Second):
			}
			continue
		}

		return nil, fmt.Errorf("gemini api returned status %d: %s", resp.StatusCode, strings.ReplaceAll(string(respBody), c.apiKey, "[REDACTED]"))
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

	if geminiResp.Candidates[0].FinishReason != "STOP" {
		return nil, fmt.Errorf("incomplete Gemini response: %s", geminiResp.Candidates[0].FinishReason)
	}
	var jsonParts strings.Builder
	for _, p := range geminiResp.Candidates[0].Content.Parts {
		jsonParts.WriteString(p.Text)
	}
	jsonText := jsonParts.String()

	var result CurriculumExtractionResult
	if err := json.Unmarshal([]byte(jsonText), &result); err != nil {
		return nil, fmt.Errorf("failed to parse structured curriculum JSON (%w): %s", err, jsonText)
	}

	if err := BindSourceCells(&result, layout); err != nil {
		return nil, err
	}
	result.SourceSHA256 = fmt.Sprintf("%x", sha256.Sum256(pdfBytes))

	return &result, nil
}

// BalanceCurriculumSemesters is retained for compatibility. Credit totals cannot
// establish a semester assignment; validation must never move modules.
// Deprecated: use BindSourceCells and ValidateCurriculum.
func BalanceCurriculumSemesters(res *CurriculumExtractionResult) {}
