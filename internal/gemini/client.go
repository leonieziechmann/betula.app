package gemini

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
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
	limiter      *Limiter
	sleep        func(context.Context, time.Duration) error // the pause between attempts without a limiter; nil: sleepContext
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

// SetLimiter paces every request of this client by l, which may be shared with
// other clients of the same API key. With a limiter, a 429 is read for the delay
// it asks for and for an exhausted daily quota, and a request that may have
// reached the API is never sent again on the client's own account (see generate).
// Without one (nil, the default) the client sends as soon as it is asked and
// retries a 429 after a fixed pause, as it always has. SummarizeModules needs a
// limiter. Set it before the client is used.
func (c *Client) SetLimiter(l *Limiter) {
	c.limiter = l
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
	MaxOutputTokens  int             `json:"maxOutputTokens,omitempty"` // zero: the model's own limit
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

	jsonText, err := c.generate(ctx, reqPayload)
	if err != nil {
		return nil, err
	}

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

// maxAttempts is how often generate sends a request the API could not serve right now.
const maxAttempts = 3

// generate sends one generateContent request and returns the text of its answer.
// It sends the API key in the x-goog-api-key header only and keeps it out of every
// error. A transient answer (503, 429) is retried up to maxAttempts times, and only
// an answer the model finished (finish reason STOP) is accepted: a truncated JSON
// document must never pass for a short one. An answer without one is an
// *answerError.
//
// With a limiter, every attempt waits for its turn first, and a 429 pauses the
// limiter for the delay the API asks for, so that no other request goes out before
// then either. A 429 for an exhausted daily quota ends the request with
// ErrDailyLimit: retrying could only fail again until the quota resets. And with a
// limiter, a request whose answer did not arrive (a timeout, a broken connection,
// a body cut off) is not sent again unless it never left: the API counts a request
// it received whether or not its answer arrives. Without a limiter, those are
// retried as they always were.
func (c *Client) generate(ctx context.Context, payload geminiRequest) (string, error) {
	bodyBytes, err := json.Marshal(payload)
	if err != nil {
		return "", fmt.Errorf("failed to marshal request: %w", err)
	}

	url := fmt.Sprintf("%s/%s:generateContent", c.baseURL, c.model)

	var respBody []byte
	var lastErr error

	for attempt := 1; attempt <= maxAttempts; attempt++ {
		if c.limiter != nil {
			if err := c.limiter.Wait(ctx); err != nil {
				return "", err
			}
		}
		httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(bodyBytes))
		if err != nil {
			return "", fmt.Errorf("failed to create http request: %w", err)
		}
		httpReq.Header.Set("Content-Type", "application/json")
		httpReq.Header.Set("x-goog-api-key", c.apiKey)

		resp, err := c.httpClient.Do(httpReq)
		if err != nil {
			lastErr = fmt.Errorf("gemini api request failed: %w", err)
			if c.limiter != nil && !neverSent(err) {
				return "", lastErr
			}
			if err := c.backOff(ctx, attempt, time.Duration(attempt)*2*time.Second); err != nil {
				return "", err
			}
			continue
		}

		respBody, err = io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			lastErr = fmt.Errorf("failed to read response: %w", err)
			if c.limiter != nil {
				return "", lastErr
			}
			continue
		}

		if resp.StatusCode == http.StatusOK {
			lastErr = nil
			break
		}

		if resp.StatusCode == http.StatusServiceUnavailable || resp.StatusCode == http.StatusTooManyRequests {
			lastErr = fmt.Errorf("gemini api temporary error %d: %s", resp.StatusCode, c.redact(respBody))
			delay := time.Duration(attempt) * 3 * time.Second
			if resp.StatusCode == http.StatusTooManyRequests && c.limiter != nil {
				quota := readQuotaError(resp.Header, respBody, c.limiter.now())
				if quota.daily {
					c.limiter.dailyQuotaUsed()
					return "", fmt.Errorf("%w (gemini api 429: %s)", ErrDailyLimit, c.redact([]byte(quota.message)))
				}
				if quota.retryDelay > maxRetryDelay {
					// The limiter refuses every request until then rather than wait (see Limiter.Wait).
					c.limiter.holdOff(quota.retryDelay)
					return "", fmt.Errorf("gemini api asks to wait %s, longer than a request waits: %w", quota.retryDelay, lastErr)
				}
				if quota.retryDelay > 0 {
					delay = quota.retryDelay
				}
				// The limiter holds every request back, this one's next attempt included.
				c.limiter.holdOff(delay)
				continue
			}
			if err := c.backOff(ctx, attempt, delay); err != nil {
				return "", err
			}
			continue
		}

		return "", fmt.Errorf("gemini api returned status %d: %s", resp.StatusCode, c.redact(respBody))
	}

	if lastErr != nil {
		return "", lastErr
	}

	var geminiResp geminiResponse
	if err := json.Unmarshal(respBody, &geminiResp); err != nil {
		return "", fmt.Errorf("failed to parse gemini response: %w", err)
	}

	if geminiResp.Error != nil {
		return "", fmt.Errorf("gemini api error %d (%s): %s", geminiResp.Error.Code, geminiResp.Error.Status, c.redact([]byte(geminiResp.Error.Message)))
	}

	if len(geminiResp.Candidates) == 0 || len(geminiResp.Candidates[0].Content.Parts) == 0 {
		return "", &answerError{"gemini returned no content candidates"}
	}

	if geminiResp.Candidates[0].FinishReason != "STOP" {
		return "", &answerError{"incomplete Gemini response: " + geminiResp.Candidates[0].FinishReason}
	}
	var jsonParts strings.Builder
	for _, p := range geminiResp.Candidates[0].Content.Parts {
		jsonParts.WriteString(p.Text)
	}
	return jsonParts.String(), nil
}

// answerError is an answer the API delivered whose model gave no usable text: no
// candidate (a blocked prompt) or one it did not finish (MAX_TOKENS, SAFETY, …).
// Asking again for the same can only fail the same way.
type answerError struct{ msg string }

func (e *answerError) Error() string { return e.msg }

// neverSent tells the error of a request that cannot have reached the API: its
// connection was never made.
func neverSent(err error) bool {
	var op *net.OpError
	return errors.As(err, &op) && op.Op == "dial"
}

// backOff waits d before the next attempt, on the limiter's clock when there is
// one. After the last attempt there is nothing to wait for, only a cancelled
// context to report.
func (c *Client) backOff(ctx context.Context, attempt int, d time.Duration) error {
	if attempt >= maxAttempts {
		return ctx.Err()
	}
	if c.limiter != nil {
		return c.limiter.sleep(ctx, d)
	}
	if c.sleep != nil {
		return c.sleep(ctx, d)
	}
	return sleepContext(ctx, d)
}

// redact removes the API key from text that goes into an error.
func (c *Client) redact(body []byte) string {
	if c.apiKey == "" {
		return string(body)
	}
	return strings.ReplaceAll(string(body), c.apiKey, "[REDACTED]")
}

// BalanceCurriculumSemesters is retained for compatibility. Credit totals cannot
// establish a semester assignment; validation must never move modules.
// Deprecated: use BindSourceCells and ValidateCurriculum.
func BalanceCurriculumSemesters(res *CurriculumExtractionResult) {}
