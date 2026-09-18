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
	"strings"
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
	SemesterSpan           string  `json:"semester_span,omitempty"` // e.g. "5-6", "3-4"
	StartSemester          int     `json:"start_semester,omitempty"`
	EndSemester            int     `json:"end_semester,omitempty"`
	Credits                float64 `json:"credits,omitempty"`
	MinCredits             float64 `json:"min_credits,omitempty"` // e.g. 10.0 for "10-24"
	MaxCredits             float64 `json:"max_credits,omitempty"` // e.g. 24.0 for "10-24"
	ModuleType             string  `json:"module_type"` // "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
	StudySection           string  `json:"study_section,omitempty"` // "Grundstudium", "Fachstudium", "Vertiefungsstudium"
	SubjectArea            string  `json:"subject_area,omitempty"`  // "Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Nebenfach", etc.
	AreaRules              string  `json:"area_rules,omitempty"`    // e.g. "Im Nebenfach müssen alle Module aus demselben Bereich gewählt werden", "Mind. 12 LP"
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
				"required": ["module_name", "module_type", "recommended_semester", "credits"]
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

	prompt := fmt.Sprintf(`Du bist ein führender Experte für deutsche Hochschulprüfungsordnungen (PO/SO) und Regelstudienpläne der BTU Cottbus-Senftenberg.
Analysiere die beigefügte Studien- und Prüfungsordnung (bzw. Satzungsänderung) extrem gründlich und fehlerfrei.
Hinweis zum Studiengang: "%s" (Dateiname: %s).

AUFGABE: Extrahiere den offiziellen Studienablaufplan / Regelstudienplan (Modultabelle / Semesterübersicht) der Anlage zur Ordnung.

BEACHTE DIE FOLGENDEN STRENGEN REGELN FÜR DIE TABELLENANALYSE:

1. SPALTEN-ZUORDNUNG & MEHRSEMESTRIGE MODULE:
   - Jede Zahl in den Spalten "1", "2", "3", "4", "5", "6" entspricht den Leistungspunkten im jeweiligen Semester.
   - Wenn ein Modul über mehrere Semester geht (z. B. Laborpraktikum "(3+3) 6" in Semester 3 und 4):
     - Trage das Anfangssemester ein: recommended_semester = 3
     - start_semester = 3, end_semester = 4, semester_span = "3-4"
     - credits = 6.0
     - remarks = "2-semestrig (3 LP im 3. FS + 3 LP im 4. FS)"

2. MEHRERE ZAHLEN IN EINER ZEILE (SPLITTE IN SEPARATE EINTRÄGE):
   - Hat eine Zeile Zahlen in mehreren Semester-Spalten (z. B. "Anwendungsfach" mit 6 LP in Spalte 3 UND 6 LP in Spalte 4), erstelle für JEDES aktive Semester einen separaten Eintrag im Array modules:
     - Eintrag A: recommended_semester = 3, credits = 6.0, module_name = "Anwendungsfach"
     - Eintrag B: recommended_semester = 4, credits = 6.0, module_name = "Anwendungsfach"

3. VERTIKALE ZUSAMMENFASSUNGEN (VERTIKAL ZUSAMMENGEFASSTE ZELLEN):
   - Wenn ein vertikal zusammengefasstes Feld über mehrere Zeilen in einem Semester steht (wie z. B. im 4. Semester Informatik: 6 LP wählbar aus einem der drei Fachstudium-Komplexe):
     - Erfasse dies als eigenen Wahlpflicht-Slot für das betreffende Semester:
       - module_name: "Wahlpflichtmodul aus den Komplexen des Fachstudiums"
       - recommended_semester: 4, credits: 6.0
       - module_type: "Wahlpflicht", study_section: "Fachstudium", subject_area: "Fachstudium Komplexe"
       - remarks: "Wählbar aus Komplex Grundlagen der Informatik, Komplex Praktische Informatik oder Komplex Angewandte und Technische Informatik (6 LP im 4. FS)"

4. MIN/MAX-RANGES & SEMESTERÜBERGREIFENDE BEREICHE (z. B. 5.-6. SEMESTER):
   - Wenn Bereiche mit Min/Max-Spannbreiten angegeben sind (z. B. "10-24" LP im 5.–6. Semester für Komplex Grundlagen, Praktische, Angewandte Informatik):
     - Erfasse die Komplexe jeweils mit:
       - recommended_semester: 5, semester_span: "5-6"
       - min_credits: 10.0, max_credits: 24.0
       - credits: 14.0 (oder rechnerischer Mittelwert)
       - area_rules: "10–24 LP im 5.–6. Fachsemester (Gesamt 10–30 LP)"
   - Zusammen mit Seminar/Praktikum (4 LP) und Bachelorarbeit (12 LP) im 5.–6. Semester ergibt sich in Summe: Summe Fachstudium Sem 5-6 = 60 LP!

5. 30-ECTS-SANITY-CHECK & SUMMENZEILEN-ABGLEICH (PFLICHT!):
   - In einem 6-semestrigen Bachelor (180 LP) MÜSSEN pro Semester ca. 30 LP herauskommen (erlaubter Bereich 28-32 LP; Sem 5-6 zusammen = 60 LP).
   - Vergleiche deine Semester-Summen mit der Fußzeile der Tabelle ("Summe Semester", "Summe Grundstudium", "Summe Fachstudium", "Summe Studium").

6. FELDER PRO MODUL:
   - module_code: Modulnummer falls angegeben (oft 5-stellig wie "12105", "11949"), sonst leer
   - module_name: Exakter deutscher Name des Moduls oder Wahlpflicht-Platzhalters
   - module_name_en: Englischer Name falls vorhanden
   - recommended_semester: Empfohlenes Fachsemester (1, 2, 3, 4, 5, 6...)
   - recommended_semester_raw: Originaltext (z. B. "1.", "2.", "3.-4.", "5.-6.")
   - semester_span: Semester-Spanne falls zutreffend (z. B. "5-6", "3-4"), sonst leer
   - start_semester / end_semester: Bei mehsemestrigen Modulen (z. B. 3 und 4)
   - credits: Leistungspunkte / ECTS für dieses Semester als Zahl (z. B. 6.0, 8.0, 10.0, 12.0)
   - min_credits / max_credits: Min- und Max-Grenzen falls Range angegeben (z. B. 10.0 und 24.0)
   - module_type: Genau einer von "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
   - study_section: "Grundstudium" (Semester 1-2 bzw. 1-4) oder "Fachstudium" / "Vertiefungsstudium"
   - subject_area: Themenbereich / Komplex
   - area_rules: Besondere Regeln laut Fußnoten/Ordnung (z. B. "Mind. 10 bis max. 24 LP aus jedem Komplex")
   - specialization: Studienrichtung / Schwerpunkt falls modulspezifisch, sonst leer
   - sws: Semesterwochenstunden falls vorhanden
   - exam_type: Art der Prüfungsleistung
   - graded: "benotet" oder "unbenotet"
   - prerequisites: Teilnahmevoraussetzungen falls in der Tabelle angegeben
   - remarks: Besondere Hinweise oder Fußnoten

Gib ausschließlich valides JSON gemäß dem Schema zurück.`, programHint, filepath.Base(pdfPath))

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

	BalanceCurriculumSemesters(&result)

	return &result, nil
}

// BalanceCurriculumSemesters performs automated balancing and validation across study semesters,
// ensuring multi-semester spans and column alignment match the ~30 ECTS per semester standard.
func BalanceCurriculumSemesters(res *CurriculumExtractionResult) {
	if res == nil || len(res.Modules) == 0 {
		return
	}

	// 1. Calculate semester sums
	semSums := make(map[int]float64)
	for _, m := range res.Modules {
		if m.RecommendedSemester > 0 {
			semSums[m.RecommendedSemester] += m.Credits
		}
	}

	// 2. Pairwise adjacent balancing for shifted single-semester modules (e.g. Sem 3 has 36, Sem 4 has 24)
	for sem := 1; sem <= 5; sem++ {
		nextSem := sem + 1
		sumCur := semSums[sem]
		sumNext := semSums[nextSem]

		diff := sumCur - 30.0
		deficit := 30.0 - sumNext

		if diff > 0 && deficit > 0 && diff == deficit {
			// Find a module in current semester with credits == diff
			for i := range res.Modules {
				m := &res.Modules[i]
				if m.RecommendedSemester == sem && m.Credits == diff && m.ModuleType != "Pflicht" {
					m.RecommendedSemester = nextSem
					semSums[sem] -= diff
					semSums[nextSem] += diff
					break
				}
			}
		}
	}

	// 3. Multi-semester span balancing (e.g. Semesters 5 and 6 with span "5-6")
	var spanModules []*ExtractedModule
	for i := range res.Modules {
		m := &res.Modules[i]
		if (m.SemesterSpan == "5-6" || m.SemesterSpan == "5 - 6" || strings.Contains(m.RecommendedSemesterRaw, "5-6") || strings.Contains(m.RecommendedSemesterRaw, "5.-6.")) && (m.RecommendedSemester == 5 || m.RecommendedSemester == 6) {
			spanModules = append(spanModules, m)
		}
	}

	if len(spanModules) > 0 {
		// Calculate non-span credits in Sem 5 and Sem 6
		nonSpan5 := 0.0
		nonSpan6 := 0.0
		for i := range res.Modules {
			m := &res.Modules[i]
			isSpan := false
			for _, sm := range spanModules {
				if sm == m {
					isSpan = true
					break
				}
			}
			if !isSpan {
				if m.RecommendedSemester == 5 {
					nonSpan5 += m.Credits
				} else if m.RecommendedSemester == 6 {
					nonSpan6 += m.Credits
				}
			}
		}

		targetSpan5 := 30.0 - nonSpan5
		targetSpan6 := 30.0 - nonSpan6

		if targetSpan5 > 0 && targetSpan6 > 0 {
			// Distribute span module credits cleanly between sem 5 and sem 6
			curSpan5 := 0.0
			for _, m := range spanModules {
				if curSpan5+m.Credits <= targetSpan5+2.0 {
					m.RecommendedSemester = 5
					curSpan5 += m.Credits
				} else {
					m.RecommendedSemester = 6
				}
			}
		}
	}
}
