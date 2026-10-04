package gemini

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync/atomic"
	"testing"
	"time"
	"unicode/utf8"
)

// geminiAnswer is a finished generateContent answer with text.
func geminiAnswer(text string) map[string]any {
	return map[string]any{"candidates": []any{map[string]any{
		"finishReason": "STOP",
		"content":      map[string]any{"parts": []any{map[string]string{"text": text}}},
	}}}
}

// answersJSON is the model's JSON for these answers.
func answersJSON(t *testing.T, answers ...summaryAnswer) string {
	t.Helper()
	b, err := json.Marshal(answers)
	if err != nil {
		t.Fatal(err)
	}
	return string(b)
}

// goodAnswer is a valid answer for the module of the request with this key.
func goodAnswer(key string) summaryAnswer {
	return summaryAnswer{
		Key:       key,
		SummaryDE: "Zusammenfassung von " + key + ".",
		SummaryEN: "Summary of " + key + ".",
		Keywords:  []string{"thermodynamik", "thermodynamics", "wärmeübertragung", "heat transfer"},
	}
}

// promptModules reads the modules a request showed the model.
func promptModules(t *testing.T, req geminiRequest) []summaryInput {
	t.Helper()
	text := req.Contents[0].Parts[0].Text
	_, list, ok := strings.Cut(text, "\nModules:\n")
	if !ok {
		t.Fatalf("no module list in the prompt:\n%s", text)
	}
	var inputs []summaryInput
	if err := json.Unmarshal([]byte(list), &inputs); err != nil {
		t.Fatalf("module list: %v\n%s", err, list)
	}
	return inputs
}

// summaryServer answers every request with handle and counts the requests.
func summaryServer(t *testing.T, handle func(w http.ResponseWriter, req geminiRequest)) (*httptest.Server, *atomic.Int32) {
	t.Helper()
	var calls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		var req geminiRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			t.Errorf("request body: %v", err)
		}
		handle(w, req)
	}))
	t.Cleanup(server.Close)
	return server, &calls
}

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

// summaryClient is a client of the server with a limiter on a fake clock.
func summaryClient(t *testing.T, baseURL string) *Client {
	t.Helper()
	c := NewClient("test-key", "test-model")
	c.SetBaseURL(baseURL)
	limiter, _ := testLimiter(t, 10, 900)
	c.SetLimiter(limiter)
	return c
}

func TestSummarizeModules(t *testing.T) {
	hashA := strings.Repeat("a1", 32)
	hashB := strings.Repeat("b2", 32)
	long := strings.Repeat("Thermodynamische Kreisprozesse und Wärmeübertragung ", 40)
	modules := []ModuleText{
		{Key: hashA, TitleDE: "Technische Thermodynamik", TitleEN: "Engineering Thermodynamics",
			Department: "Fakultät 3", Contents: long, Outcomes: "Die Studierenden\n\n  können   Kreisprozesse berechnen."},
		{Key: hashB, TitleDE: "Masterarbeit"},
	}

	var request geminiRequest
	server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		request = req
		second := goodAnswer("m2")
		second.Keywords = []string{" masterarbeit ", "master's thesis", "", "Masterarbeit", "abschlussarbeit\n"}
		// In another order, and with an answer for a module nobody asked about.
		writeJSON(w, http.StatusOK, geminiAnswer(answersJSON(t, second, goodAnswer("m3"), goodAnswer("m1"))))
	})

	got, err := summaryClient(t, server.URL).SummarizeModules(context.Background(), modules)
	if err != nil {
		t.Fatal(err)
	}
	if calls.Load() != 1 {
		t.Fatalf("%d requests, want 1", calls.Load())
	}
	want := []ModuleSummary{
		{Key: hashA, SummaryDE: "Zusammenfassung von m1.", SummaryEN: "Summary of m1.",
			Keywords: []string{"thermodynamik", "thermodynamics", "wärmeübertragung", "heat transfer"}},
		{Key: hashB, SummaryDE: "Zusammenfassung von m2.", SummaryEN: "Summary of m2.",
			Keywords: []string{"masterarbeit", "master's thesis", "abschlussarbeit"}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got  %+v\nwant %+v", got, want)
	}

	cfg := request.GenerationConfig
	if cfg == nil || cfg.Temperature != 0 || cfg.ResponseMimeType != "application/json" ||
		cfg.MaxOutputTokens != outputTokenMargin+2*summaryTokens {
		t.Fatalf("generation config: %+v", cfg)
	}
	var schema struct {
		Type  string `json:"type"`
		Items struct {
			Required []string `json:"required"`
		} `json:"items"`
	}
	if err := json.Unmarshal(cfg.ResponseSchema, &schema); err != nil || schema.Type != "ARRAY" ||
		!reflect.DeepEqual(schema.Items.Required, []string{"key", "summary_de", "summary_en", "keywords"}) {
		t.Fatalf("response schema (%v): %s", err, cfg.ResponseSchema)
	}

	prompt := request.Contents[0].Parts[0].Text
	for _, asked := range []string{
		"1–2 sentences of plain German, at most 45 words",
		"what the module is about and what one learns or can do afterwards",
		"concrete topics, methods, tools and materials",
		"No boilerplate („Die Studierenden …“), no credits, exams or organisation",
		"summary_en: the same content in English",
		"8–15 search terms",
		"synonyms, broader and narrower terms, the field, German and English terms where they differ",
		"(KI/AI, ML, BWL, CAD, FEM, SPS, …)",
		"Lower case except abbreviations and proper names",
		"Modul, Studium, Kompetenz, and Grundlagen or Seminar on their own",
		"Write only what the module's text supports",
		"work from the titles and the department and keep it short",
		"a thesis, an internship, a project or a language course",
	} {
		if !strings.Contains(prompt, asked) {
			t.Errorf("the prompt does not ask for %q", asked)
		}
	}
	if strings.Contains(prompt, hashA) || strings.Contains(prompt, hashB) {
		t.Error("the caller's keys reached the model")
	}

	inputs := promptModules(t, request)
	if len(inputs) != 2 || inputs[0].Key != "m1" || inputs[1].Key != "m2" {
		t.Fatalf("modules of the prompt: %+v", inputs)
	}
	first := inputs[0]
	if first.TitleDE != "Technische Thermodynamik" || first.TitleEN != "Engineering Thermodynamics" || first.Department != "Fakultät 3" {
		t.Errorf("titles and department: %+v", first)
	}
	if n := utf8.RuneCountInString(first.Contents); n > maxContentsChars+2 || n < maxContentsChars-60 {
		t.Errorf("contents of %d characters, want about %d", n, maxContentsChars)
	}
	if !strings.HasSuffix(first.Contents, "Wärmeübertragung …") {
		t.Errorf("contents not cut at the end of a word: …%s", first.Contents[len(first.Contents)-40:])
	}
	if first.Outcomes != "Die Studierenden können Kreisprozesse berechnen." {
		t.Errorf("outcomes: %q", first.Outcomes)
	}
	if inputs[1].Contents != "" || inputs[1].TitleEN != "" {
		t.Errorf("empty fields sent: %+v", inputs[1])
	}
}

func TestSummarizeModulesSendsTheKeyOnlyInTheHeader(t *testing.T) {
	const apiKey = "AIzaSy-secret-key-0123"
	modules := []ModuleText{{Key: "k1", TitleDE: "Mathematik I"}}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		if got := r.Header.Get("x-goog-api-key"); got != apiKey {
			t.Errorf("x-goog-api-key %q", got)
		}
		if strings.Contains(r.URL.String(), apiKey) || r.URL.Query().Has("key") {
			t.Errorf("the key is in the URL: %s", r.URL)
		}
		if strings.Contains(string(body), apiKey) {
			t.Error("the key is in the body")
		}
		for name, values := range r.Header {
			if name != "X-Goog-Api-Key" && strings.Contains(strings.Join(values, " "), apiKey) {
				t.Errorf("the key is in the header %s", name)
			}
		}
		if r.URL.Path != "/test-model:generateContent" || r.Method != http.MethodPost {
			t.Errorf("%s %s", r.Method, r.URL.Path)
		}
		// An API that echoes the key back must not get it into an error.
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": map[string]any{"code": 400, "message": "API key not valid: " + apiKey}})
	}))
	defer server.Close()

	c := NewClient(apiKey, "test-model")
	c.SetBaseURL(server.URL)
	limiter, _ := testLimiter(t, 10, 900)
	c.SetLimiter(limiter)
	_, err := c.SummarizeModules(context.Background(), modules)
	if err == nil {
		t.Fatal("a 400 passed")
	}
	if strings.Contains(err.Error(), apiKey) || !strings.Contains(err.Error(), "[REDACTED]") {
		t.Fatalf("the key is not redacted: %v", err)
	}

	// Nor an error in a 200.
	echo := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]any{"error": map[string]any{"code": 400, "message": "API key not valid: " + apiKey}})
	}))
	defer echo.Close()
	c.SetBaseURL(echo.URL)
	_, err = c.SummarizeModules(context.Background(), modules)
	if err == nil || strings.Contains(err.Error(), apiKey) || !strings.Contains(err.Error(), "[REDACTED]") {
		t.Fatalf("the key is not redacted in an error of a 200: %v", err)
	}
}

func TestSummarizeModulesReportsTheModulesWithoutAValidAnswer(t *testing.T) {
	var modules []ModuleText
	for i := 1; i <= 7; i++ {
		modules = append(modules, ModuleText{Key: fmt.Sprintf("hash%d", i), TitleDE: fmt.Sprintf("Modul %d", i)})
	}
	server, _ := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		emptyDE := goodAnswer("m4")
		emptyDE.SummaryDE = "  \n"
		fewKeywords := goodAnswer("m5")
		fewKeywords.Keywords = []string{"thermodynamik", "", "Thermodynamik", "heat"}
		longEN := goodAnswer("m6")
		longEN.SummaryEN = strings.Repeat("word ", 130)
		answers := answersJSON(t,
			goodAnswer(" m1 "),                 // keys are trimmed
			goodAnswer("m3"), goodAnswer("m3"), // m2 missing, m3 twice
			emptyDE, fewKeywords, longEN,
			goodAnswer("m7"),
			goodAnswer("hash2"), // the caller's key is not a key of the request
		)
		writeJSON(w, http.StatusOK, geminiAnswer(answers))
	})

	got, err := summaryClient(t, server.URL).SummarizeModules(context.Background(), modules)
	var incomplete *IncompleteError
	if !errors.As(err, &incomplete) {
		t.Fatalf("got %v, want an *IncompleteError", err)
	}
	if !reflect.DeepEqual(incomplete.Keys, []string{"hash2", "hash3", "hash4", "hash5", "hash6"}) {
		t.Fatalf("failed keys %v (%v)", incomplete.Keys, err)
	}
	reasons := incomplete.Reasons
	for key, want := range map[string]string{"hash2": "no answer", "hash3": "answered 2 times",
		"hash4": "summary_de empty", "hash5": "2 keywords", "hash6": "summary_en of 649 characters"} {
		if reasons[key] != want {
			t.Errorf("%s: %q, want %q", key, reasons[key], want)
		}
	}
	if len(got) != 2 || got[0].Key != "hash1" || got[1].Key != "hash7" {
		t.Fatalf("valid summaries %+v, want hash1 and hash7", got)
	}
	if !strings.Contains(err.Error(), "5 module summaries missing or invalid: hash2 (no answer); hash3") {
		t.Errorf("message: %v", err)
	}
}

// An answer that fails as a whole names every module of the request, so that the
// caller goes on with the next batch instead of asking for this one again and again.
func TestSummarizeModulesNamesEveryModuleOfAnUnusableAnswer(t *testing.T) {
	notJSON := `[{"key":"m1","summary_de":"Zusammen` + strings.Repeat("fassung ", 2000)
	for _, tc := range []struct {
		name   string
		answer map[string]any
		reason string
	}{
		{"not JSON", geminiAnswer(notJSON), fmt.Sprintf("answer of %d bytes is not the JSON asked for", len(notJSON))},
		{"MAX_TOKENS", withFinishReason(geminiAnswer(answersJSON(t, goodAnswer("m1"), goodAnswer("m2"))), "MAX_TOKENS"), "MAX_TOKENS"},
		{"SAFETY", withFinishReason(geminiAnswer("[]"), "SAFETY"), "SAFETY"},
		{"blocked prompt", map[string]any{"promptFeedback": map[string]any{"blockReason": "OTHER"}}, "no content candidates"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
				writeJSON(w, http.StatusOK, tc.answer)
			})
			modules := []ModuleText{{Key: "k1", TitleDE: "Physik"}, {Key: "k2", TitleDE: "Chemie"}}
			got, err := summaryClient(t, server.URL).SummarizeModules(context.Background(), modules)
			var incomplete *IncompleteError
			if !errors.As(err, &incomplete) || got != nil || calls.Load() != 1 {
				t.Fatalf("got %v and %v after %d requests, want an *IncompleteError after one", got, err, calls.Load())
			}
			if !reflect.DeepEqual(incomplete.Keys, []string{"k1", "k2"}) {
				t.Fatalf("failed keys %v", incomplete.Keys)
			}
			for _, key := range incomplete.Keys {
				if r := incomplete.Reasons[key]; !strings.Contains(r, tc.reason) || len(r) > 200 {
					t.Errorf("%s: reason %q, want one with %q", key, r, tc.reason)
				}
			}
		})
	}
}

func withFinishReason(answer map[string]any, reason string) map[string]any {
	answer["candidates"].([]any)[0].(map[string]any)["finishReason"] = reason
	return answer
}

func TestSummarizeModulesChecksTheRequest(t *testing.T) {
	server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		t.Error("a request went out")
	})
	c := summaryClient(t, server.URL)
	ctx := context.Background()

	if got, err := c.SummarizeModules(ctx, nil); got != nil || err != nil {
		t.Errorf("no modules: %v, %v", got, err)
	}
	tooMany := make([]ModuleText, MaxSummaryBatch+1)
	for i := range tooMany {
		tooMany[i] = ModuleText{Key: fmt.Sprint(i), TitleDE: "Modul"}
	}
	if _, err := c.SummarizeModules(ctx, tooMany); err == nil {
		t.Errorf("%d modules accepted", len(tooMany))
	}
	if _, err := c.SummarizeModules(ctx, []ModuleText{{Key: "x"}, {Key: "x"}}); err == nil {
		t.Error("a key twice accepted")
	}
	if _, err := c.SummarizeModules(ctx, []ModuleText{{TitleDE: "Modul"}}); err == nil {
		t.Error("a module without a key accepted")
	}
	noKey := NewClient("", "")
	noKey.SetBaseURL(server.URL)
	if _, err := noKey.SummarizeModules(ctx, []ModuleText{{Key: "x"}}); !errors.Is(err, ErrNoAPIKey) {
		t.Errorf("without an API key: %v", err)
	}
	noLimiter := NewClient("test-key", "test-model")
	noLimiter.SetBaseURL(server.URL)
	if _, err := noLimiter.SummarizeModules(ctx, []ModuleText{{Key: "x"}}); !errors.Is(err, ErrNoLimiter) {
		t.Errorf("without a limiter: %v", err)
	}
	if calls.Load() != 0 {
		t.Fatalf("%d requests", calls.Load())
	}
}

// quota429 is the API's answer for a used-up quota.
func quota429(quotaID, retryDelay string) map[string]any {
	details := []any{map[string]any{
		"@type":      "type.googleapis.com/google.rpc.QuotaFailure",
		"violations": []any{map[string]any{"quotaId": quotaID, "quotaValue": "10"}},
	}}
	if retryDelay != "" {
		details = append(details, map[string]any{"@type": "type.googleapis.com/google.rpc.RetryInfo", "retryDelay": retryDelay})
	}
	return map[string]any{"error": map[string]any{"code": 429, "status": "RESOURCE_EXHAUSTED",
		"message": "You exceeded your current quota.", "details": details}}
}

func TestSummarizeModulesWaitsTheDelayA429AsksFor(t *testing.T) {
	for _, tc := range []struct {
		name   string
		answer func(w http.ResponseWriter)
		want   time.Duration
	}{
		{"RetryInfo", func(w http.ResponseWriter) {
			writeJSON(w, http.StatusTooManyRequests, quota429("GenerateRequestsPerMinutePerProjectPerModel-FreeTier", "37s"))
		}, 37 * time.Second},
		{"Retry-After", func(w http.ResponseWriter) {
			w.Header().Set("Retry-After", "20")
			writeJSON(w, http.StatusTooManyRequests, map[string]any{"error": map[string]any{"code": 429}})
		}, 20 * time.Second},
		{"neither", func(w http.ResponseWriter) {
			writeJSON(w, http.StatusTooManyRequests, map[string]any{"error": map[string]any{"code": 429}})
		}, 3 * time.Second},
	} {
		t.Run(tc.name, func(t *testing.T) {
			limiter, clock := testLimiter(t, 10, 900)
			var sentAt []time.Time
			server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
				sentAt = append(sentAt, clock.now())
				if len(sentAt) == 1 {
					tc.answer(w)
					return
				}
				writeJSON(w, http.StatusOK, geminiAnswer(answersJSON(t, goodAnswer("m1"))))
			})
			c := summaryClient(t, server.URL)
			c.SetLimiter(limiter)

			got, err := c.SummarizeModules(context.Background(), []ModuleText{{Key: "k", TitleDE: "Physik"}})
			if err != nil {
				t.Fatal(err)
			}
			if len(got) != 1 || calls.Load() != 2 {
				t.Fatalf("%d summaries after %d requests", len(got), calls.Load())
			}
			if gap := sentAt[1].Sub(sentAt[0]); gap != tc.want {
				t.Fatalf("retried after %s, want %s", gap, tc.want)
			}
		})
	}
}

func TestSummarizeModulesStopsAtTheDailyQuota(t *testing.T) {
	limiter, clock := testLimiter(t, 10, 900)
	server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		writeJSON(w, http.StatusTooManyRequests, quota429("GenerateRequestsPerDayPerProjectPerModel-FreeTier", "4s"))
	})
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	modules := []ModuleText{{Key: "k", TitleDE: "Physik"}}

	_, err := c.SummarizeModules(context.Background(), modules)
	if !errors.Is(err, ErrDailyLimit) {
		t.Fatalf("got %v, want ErrDailyLimit", err)
	}
	if calls.Load() != 1 || clock.total() != 0 {
		t.Fatalf("%d requests and %s waited, want one request and no wait", calls.Load(), clock.total())
	}
	// The next batch does not even ask.
	if _, err := c.SummarizeModules(context.Background(), modules); !errors.Is(err, ErrDailyLimit) || calls.Load() != 1 {
		t.Fatalf("second batch: %v after %d requests", err, calls.Load())
	}
}

func TestSummarizeModulesGivesUpOnAnHoursLongDelay(t *testing.T) {
	limiter, clock := testLimiter(t, 10, 900)
	server, calls := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		writeJSON(w, http.StatusTooManyRequests, quota429("SomeOtherQuota", "3600s"))
	})
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	modules := []ModuleText{{Key: "k", TitleDE: "Physik"}}
	_, err := c.SummarizeModules(context.Background(), modules)
	if err == nil || errors.Is(err, ErrDailyLimit) || calls.Load() != 1 || clock.total() != 0 {
		t.Fatalf("got %v after %d requests and %s", err, calls.Load(), clock.total())
	}
	// Until the hour is over, the next batch is refused at once, not sent or kept waiting.
	clock.advance(50 * time.Minute)
	if _, err := c.SummarizeModules(context.Background(), modules); err == nil || calls.Load() != 1 || clock.total() != 0 {
		t.Fatalf("within the hour: %v after %d requests and %s", err, calls.Load(), clock.total())
	}
	// Within the last two minutes, it waits for the rest.
	clock.advance(9 * time.Minute)
	if _, err := c.SummarizeModules(context.Background(), modules); calls.Load() != 2 || clock.total() != time.Minute {
		t.Fatalf("at the end of the hour: %v after %d requests and %s", err, calls.Load(), clock.total())
	}
}

func TestSummarizeModulesIsPacedByTheLimiter(t *testing.T) {
	limiter, clock := testLimiter(t, 2, 3)
	var sentAt []time.Time
	server, _ := summaryServer(t, func(w http.ResponseWriter, req geminiRequest) {
		sentAt = append(sentAt, clock.now())
		writeJSON(w, http.StatusOK, geminiAnswer(answersJSON(t, goodAnswer("m1"))))
	})
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	modules := []ModuleText{{Key: "k", TitleDE: "Physik"}}
	for i := range 3 {
		if _, err := c.SummarizeModules(context.Background(), modules); err != nil {
			t.Fatalf("batch %d: %v", i+1, err)
		}
	}
	if len(sentAt) != 3 || sentAt[1] != sentAt[0] || sentAt[2].Sub(sentAt[0]) != time.Minute {
		t.Fatalf("sent at %v", sentAt)
	}
	if _, err := c.SummarizeModules(context.Background(), modules); !errors.Is(err, ErrDailyLimit) || len(sentAt) != 3 {
		t.Fatalf("fourth batch: %v", err)
	}
}

// countingServer serves handle, which learns the number of its request, and counts
// the requests.
func countingServer(t *testing.T, handle func(w http.ResponseWriter, r *http.Request, n int32)) (*httptest.Server, *atomic.Int32) {
	t.Helper()
	var calls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = io.Copy(io.Discard, r.Body)
		handle(w, r, calls.Add(1))
	}))
	t.Cleanup(server.Close)
	return server, &calls
}

// brokenBody answers with status and a body that breaks off.
func brokenBody(t *testing.T, w http.ResponseWriter, status int) {
	conn, buf, err := w.(http.Hijacker).Hijack()
	if err != nil {
		t.Error(err)
		return
	}
	defer conn.Close()
	fmt.Fprintf(buf, "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{\"candidates\":[", status, http.StatusText(status))
	_ = buf.Flush()
}

// tooSlow answers after the client has given up.
func tooSlow(r *http.Request) {
	select {
	case <-r.Context().Done():
	case <-time.After(5 * time.Second):
	}
}

// With a limiter, a request the API may have received and counted is not sent
// again: neither after a timeout nor after an answer whose body broke off.
func TestGenerateWithALimiterNeverResendsARequestThatMayHaveArrived(t *testing.T) {
	for _, tc := range []struct {
		name   string
		handle func(t *testing.T, w http.ResponseWriter, r *http.Request)
	}{
		{"timeout", func(t *testing.T, w http.ResponseWriter, r *http.Request) { tooSlow(r) }},
		{"200 cut off", func(t *testing.T, w http.ResponseWriter, r *http.Request) { brokenBody(t, w, http.StatusOK) }},
		{"503 cut off", func(t *testing.T, w http.ResponseWriter, r *http.Request) {
			brokenBody(t, w, http.StatusServiceUnavailable)
		}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, _ int32) { tc.handle(t, w, r) })
			c := summaryClient(t, server.URL)
			c.httpClient.Timeout = 50 * time.Millisecond
			_, err := c.SummarizeModules(context.Background(), []ModuleText{{Key: "k", TitleDE: "Physik"}})
			var incomplete *IncompleteError
			if err == nil || errors.As(err, &incomplete) || calls.Load() != 1 || c.limiter.today != 1 {
				t.Fatalf("got %v after %d requests (%d counted), want a plain error after one", err, calls.Load(), c.limiter.today)
			}
		})
	}
}

// A request that never left, as when the connection is refused, is retried.
func TestGenerateWithALimiterRetriesARequestThatNeverLeft(t *testing.T) {
	server := httptest.NewServer(http.NotFoundHandler())
	server.Close()
	limiter, clock := testLimiter(t, 10, 900)
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	_, err := c.SummarizeModules(context.Background(), []ModuleText{{Key: "k", TitleDE: "Physik"}})
	if err == nil || limiter.today != maxAttempts || !reflect.DeepEqual(clock.slept, []time.Duration{2 * time.Second, 4 * time.Second}) {
		t.Fatalf("got %v after %d attempts and pauses %v", err, limiter.today, clock.slept)
	}
}

func TestGenerateRetriesA503ThroughTheLimiter(t *testing.T) {
	limiter, clock := testLimiter(t, 1, 900)
	var sentAt []time.Time
	server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, n int32) {
		sentAt = append(sentAt, clock.now())
		if n == 1 {
			writeJSON(w, http.StatusServiceUnavailable, map[string]any{"error": map[string]any{"code": 503}})
			return
		}
		writeJSON(w, http.StatusOK, geminiAnswer(answersJSON(t, goodAnswer("m1"))))
	})
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	got, err := c.SummarizeModules(context.Background(), []ModuleText{{Key: "k", TitleDE: "Physik"}})
	if err != nil || len(got) != 1 || calls.Load() != 2 {
		t.Fatalf("got %v and %v after %d requests", got, err, calls.Load())
	}
	// The pause of 3s, then the rest of the minute that the one request a minute takes.
	if gap := sentAt[1].Sub(sentAt[0]); gap != time.Minute || limiter.today != 2 ||
		!reflect.DeepEqual(clock.slept, []time.Duration{3 * time.Second, 57 * time.Second}) {
		t.Fatalf("retried after %s (pauses %v, %d counted)", gap, clock.slept, limiter.today)
	}
}

func TestGenerateSendsNoFourthRequestForA429(t *testing.T) {
	limiter, clock := testLimiter(t, 10, 900)
	server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, _ int32) {
		writeJSON(w, http.StatusTooManyRequests, quota429("GenerateRequestsPerMinutePerProjectPerModel-FreeTier", "10s"))
	})
	c := summaryClient(t, server.URL)
	c.SetLimiter(limiter)
	_, err := c.SummarizeModules(context.Background(), []ModuleText{{Key: "k", TitleDE: "Physik"}})
	if err == nil || errors.Is(err, ErrDailyLimit) || !strings.Contains(err.Error(), "429") {
		t.Fatalf("got %v", err)
	}
	if calls.Load() != maxAttempts || limiter.today != maxAttempts || clock.total() != 20*time.Second {
		t.Fatalf("%d requests (%d counted) and %s waited, want 3 and 20s", calls.Load(), limiter.today, clock.total())
	}
}

func TestGenerateEndsWithTheContext(t *testing.T) {
	for _, limited := range []bool{true, false} {
		for _, tc := range []struct {
			name   string
			handle func(w http.ResponseWriter, r *http.Request, cancel context.CancelFunc)
		}{
			{"during the pause", func(w http.ResponseWriter, r *http.Request, cancel context.CancelFunc) {
				cancel()
				writeJSON(w, http.StatusServiceUnavailable, map[string]any{"error": map[string]any{"code": 503}})
			}},
			{"in flight", func(w http.ResponseWriter, r *http.Request, cancel context.CancelFunc) {
				cancel()
				tooSlow(r)
			}},
		} {
			t.Run(fmt.Sprintf("%s/limiter=%v", tc.name, limited), func(t *testing.T) {
				ctx, cancel := context.WithCancel(context.Background())
				defer cancel()
				server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, _ int32) { tc.handle(w, r, cancel) })
				c := NewClient("test-key", "test-model")
				c.SetBaseURL(server.URL)
				c.sleep = (&fakeClock{}).sleep
				if limited {
					limiter, _ := testLimiter(t, 10, 900)
					c.SetLimiter(limiter)
				}
				_, err := c.generate(ctx, geminiRequest{})
				if !errors.Is(err, context.Canceled) || calls.Load() != 1 {
					t.Fatalf("got %v after %d requests, want context.Canceled after one", err, calls.Load())
				}
			})
		}
	}
}

// extractClient is a client for ExtractCurriculumFromPDF without a limiter, whose
// pauses between attempts are recorded instead of waited.
func extractClient(t *testing.T, baseURL string) (c *Client, pdf string, clock *fakeClock) {
	t.Helper()
	pdf = filepath.Join(t.TempDir(), "test.pdf")
	if err := os.WriteFile(pdf, []byte("%PDF"), 0o644); err != nil {
		t.Fatal(err)
	}
	c = NewClient("test-key", "test-model")
	c.SetBaseURL(baseURL)
	c.layoutLoader = func(context.Context, string) (*PDFLayout, error) {
		return &PDFLayout{Cells: []SourceCell{{ID: "a", Row: "Physik", Semesters: []int{1}, Min: 5, Max: 5}}}, nil
	}
	clock = &fakeClock{}
	c.sleep = clock.sleep
	return c, pdf, clock
}

// Without a limiter, ExtractCurriculumFromPDF retries as it always has.
func TestExtractCurriculumFromPDFRetriesAsBefore(t *testing.T) {
	const curriculum = `{"program_name":"Test","modules":[{"source_cell":"a","module_name":"Physik","recommended_semester":1,"credits":5,"module_type":"Pflicht"}]}`
	t.Run("503", func(t *testing.T) {
		server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, n int32) {
			if n == 1 {
				writeJSON(w, http.StatusServiceUnavailable, map[string]any{"error": map[string]any{"code": 503}})
				return
			}
			writeJSON(w, http.StatusOK, geminiAnswer(curriculum))
		})
		c, pdf, clock := extractClient(t, server.URL)
		res, err := c.ExtractCurriculumFromPDF(context.Background(), pdf, "Test")
		if err != nil || res.ProgramName != "Test" || calls.Load() != 2 || !reflect.DeepEqual(clock.slept, []time.Duration{3 * time.Second}) {
			t.Fatalf("got %v after %d requests and pauses %v", err, calls.Load(), clock.slept)
		}
	})
	t.Run("body cut off", func(t *testing.T) {
		server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, _ int32) { brokenBody(t, w, http.StatusOK) })
		c, pdf, clock := extractClient(t, server.URL)
		if _, err := c.ExtractCurriculumFromPDF(context.Background(), pdf, "Test"); err == nil || calls.Load() != maxAttempts || len(clock.slept) != 0 {
			t.Fatalf("got %v after %d requests and pauses %v", err, calls.Load(), clock.slept)
		}
	})
	t.Run("timeout", func(t *testing.T) {
		server, calls := countingServer(t, func(w http.ResponseWriter, r *http.Request, _ int32) { tooSlow(r) })
		c, pdf, clock := extractClient(t, server.URL)
		c.httpClient.Timeout = 50 * time.Millisecond
		_, err := c.ExtractCurriculumFromPDF(context.Background(), pdf, "Test")
		if err == nil || calls.Load() != maxAttempts || !reflect.DeepEqual(clock.slept, []time.Duration{2 * time.Second, 4 * time.Second}) {
			t.Fatalf("got %v after %d requests and pauses %v", err, calls.Load(), clock.slept)
		}
	})
	t.Run("no output cap", func(t *testing.T) {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			body, _ := io.ReadAll(r.Body)
			if strings.Contains(string(body), "maxOutputTokens") {
				t.Error("the curriculum request has an output cap")
			}
			writeJSON(w, http.StatusOK, geminiAnswer(curriculum))
		}))
		defer server.Close()
		c, pdf, _ := extractClient(t, server.URL)
		if _, err := c.ExtractCurriculumFromPDF(context.Background(), pdf, "Test"); err != nil {
			t.Fatal(err)
		}
	})
}

func TestCheckSummaryDropsInvisibleAndOverlongText(t *testing.T) {
	a := goodAnswer("m1")
	a.Keywords = append([]string{"\u200b", "\u00ad \u200b", strings.Repeat("überlang ", 10)}, a.Keywords[:3]...)
	s, reason := checkSummary(a)
	if reason != "" || !reflect.DeepEqual(s.Keywords, []string{"thermodynamik", "thermodynamics", "wärmeübertragung"}) {
		t.Fatalf("got %q and keywords %q", reason, s.Keywords)
	}
	a.Keywords = []string{"\u200b", "\u00ad", "\ufeff\u200b"}
	if _, reason := checkSummary(a); reason != "0 keywords" {
		t.Fatalf("invisible keywords: %q", reason)
	}
	a = goodAnswer("m1")
	a.SummaryDE = "\u200b\u00ad"
	if _, reason := checkSummary(a); reason != "summary_de empty" {
		t.Fatalf("an invisible summary: %q", reason)
	}
	if got := oneLine("Thermo\u00addynamik\u200b und  Wärme"); got != "Thermodynamik und Wärme" {
		t.Fatalf("oneLine: %q", got)
	}
}

func TestClip(t *testing.T) {
	for _, tc := range []struct {
		in   string
		max  int
		want string
	}{
		{"kurz", 10, "kurz"},
		{"  zwei\n\nZeilen  ", 20, "zwei Zeilen"},
		{"genau zehn", 10, "genau zehn"},
		{"eins zwei drei vier", 10, "eins zwei …"},                    // the cut falls within „drei“
		{"eins zweii drei", 10, "eins zweii …"},                       // the cut falls on the space after „zweii“
		{"eins zwei, drei", 10, "eins zwei …"},                        // no comma before the mark
		{"Donaudampfschifffahrt", 10, "Donaudampf …"},                 // one word: cut within
		{"ab Donaudampfschifffahrt", 10, "ab Donauda …"},              // the word reaches back beyond half
		{"Übungsaufgaben über Ökologie", 20, "Übungsaufgaben über …"}, // characters, not bytes
	} {
		if got := clip(tc.in, tc.max); got != tc.want {
			t.Errorf("clip(%q, %d) = %q, want %q", tc.in, tc.max, got, tc.want)
		}
	}
}
