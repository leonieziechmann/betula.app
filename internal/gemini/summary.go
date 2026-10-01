package gemini

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

// The summarizer writes, for the semantic search, what a module is about in the
// words a student would search for. Its summaries are embedded with the module's
// text and never published: they are not the university's text.

// ModuleText is a module as the summarizer reads it. Key is the caller's hash of
// the text; it comes back in the summary and is never shown to the model.
type ModuleText struct {
	Key        string
	TitleDE    string
	TitleEN    string
	Department string
	Contents   string
	Outcomes   string
}

// ModuleSummary is what the model wrote about one module: a German and an
// English summary and the search terms for it.
type ModuleSummary struct {
	Key       string
	SummaryDE string
	SummaryEN string
	Keywords  []string
}

// MaxSummaryBatch is the most modules SummarizeModules takes in one request. A
// module makes about 500 tokens of input and 200 of output, so a full batch about
// 12,500 and 5,000. A larger batch loses more to one answer that fails, and it
// nears the free tier's tokens per minute.
const MaxSummaryBatch = 25

// The output a request may take: summaryTokens for each module, about twice what
// an answer as the prompt asks for needs, and outputTokenMargin for the model's
// thinking, which counts as output. The cap ends an answer that loops, as one at
// temperature 0 may, as MAX_TOKENS within the client's timeout, rather than run on
// to the model's own limit and past the timeout.
const (
	summaryTokens     = 400
	outputTokenMargin = 8192
)

// How much of a module's text goes to the model. Most module descriptions say
// what they are about in their first paragraphs; the rest is detail the summary
// has no room for, and tokens of the free tier.
const (
	maxContentsChars = 1100
	maxOutcomesChars = 500
)

// The bounds of a valid answer. The prompt asks for at most 45 words and 8–15
// terms; the bounds are wider, so that only an answer that went wrong fails.
const (
	maxSummaryChars = 600
	minKeywords     = 3
	maxKeywords     = 20
	maxKeywordChars = 60
)

// ErrNoAPIKey says that the client has no API key; summaries need the API.
var ErrNoAPIKey = errors.New("gemini: no API key")

// ErrNoLimiter says that the client has no Limiter (Client.SetLimiter). Summaries
// are asked for in a loop, which only a limiter paces and stops at the daily quota.
var ErrNoLimiter = errors.New("gemini: module summaries need a limiter (Client.SetLimiter)")

// IncompleteError lists the modules of a request whose answer was missing or
// invalid. SummarizeModules returns it with the summaries that were valid, so
// that the caller keeps those and asks again for these.
type IncompleteError struct {
	Keys    []string          // the caller's keys, in the order of the request
	Reasons map[string]string // what was wrong, by key
}

func (e *IncompleteError) Error() string {
	var b strings.Builder
	fmt.Fprintf(&b, "gemini: %d module summaries missing or invalid", len(e.Keys))
	for i, key := range e.Keys {
		sep := "; "
		if i == 0 {
			sep = ": "
		}
		fmt.Fprintf(&b, "%s%s (%s)", sep, key, e.Reasons[key])
	}
	return b.String()
}

const summarySchema = `{
	"type": "ARRAY",
	"items": {
		"type": "OBJECT",
		"properties": {
			"key": { "type": "STRING" },
			"summary_de": { "type": "STRING" },
			"summary_en": { "type": "STRING" },
			"keywords": { "type": "ARRAY", "items": { "type": "STRING" } }
		},
		"required": ["key", "summary_de", "summary_en", "keywords"],
		"propertyOrdering": ["key", "summary_de", "summary_en", "keywords"]
	}
}`

const summaryPrompt = `Below is a JSON array of university modules (German titles, often an English title, the department, and the module's description of its contents and learning outcomes). For each module, write a search summary. The summaries are not shown to anyone: they are embedded for a semantic search over the modules, so they must name what a student would search for.

Answer with one object per module and copy its "key" exactly.
- summary_de: 1–2 sentences of plain German, at most 45 words: what the module is about and what one learns or can do afterwards. Name the concrete topics, methods, tools and materials. No boilerplate („Die Studierenden …“), no credits, exams or organisation.
- summary_en: the same content in English.
- keywords: 8–15 search terms a student might use: the core topics, synonyms, broader and narrower terms, the field, German and English terms where they differ, common abbreviations (KI/AI, ML, BWL, CAD, FEM, SPS, …). Lower case except abbreviations and proper names. No generic words (Modul, Studium, Kompetenz, and Grundlagen or Seminar on their own).

Write only what the module's text supports. If the text is empty or says little, work from the titles and the department and keep it short. For a thesis, an internship, a project or a language course, say that it is one.

Modules:
`

// summaryInput is a module as the prompt shows it.
type summaryInput struct {
	Key        string `json:"key"`
	TitleDE    string `json:"title_de,omitempty"`
	TitleEN    string `json:"title_en,omitempty"`
	Department string `json:"department,omitempty"`
	Contents   string `json:"contents,omitempty"`
	Outcomes   string `json:"outcomes,omitempty"`
}

// summaryAnswer is one object of the model's answer.
type summaryAnswer struct {
	Key       string   `json:"key"`
	SummaryDE string   `json:"summary_de"`
	SummaryEN string   `json:"summary_en"`
	Keywords  []string `json:"keywords"`
}

// SummarizeModules asks the model, in one request, for a summary of each module,
// at most MaxSummaryBatch of them. The model sees each module under a short key
// of the request ("m1", "m2", …) rather than the caller's: a hash of 64 hex digits
// is a thing a model copies wrong.
//
// The summaries come back in the order of modules. A module whose answer is
// missing, given twice or invalid is left out and named in an *IncompleteError; the
// others are valid and returned with it. An answer the model did not finish
// (MAX_TOKENS, SAFETY, a blocked prompt) or that is not the JSON asked for names
// every module so: asking again may fail the same way, but must not hold up the
// other modules. Any other error means no summaries: the request failed, or the
// day's requests are used up (ErrDailyLimit). The client needs a limiter
// (ErrNoLimiter).
func (c *Client) SummarizeModules(ctx context.Context, modules []ModuleText) ([]ModuleSummary, error) {
	if len(modules) == 0 {
		return nil, nil
	}
	if len(modules) > MaxSummaryBatch {
		return nil, fmt.Errorf("gemini: %d modules in one summary request, at most %d", len(modules), MaxSummaryBatch)
	}
	if c.apiKey == "" {
		return nil, ErrNoAPIKey
	}
	if c.limiter == nil {
		return nil, ErrNoLimiter
	}

	inputs := make([]summaryInput, len(modules))
	seen := make(map[string]bool, len(modules))
	for i, m := range modules {
		if m.Key == "" {
			return nil, fmt.Errorf("gemini: module %d of the summary request has no key", i+1)
		}
		if seen[m.Key] {
			return nil, fmt.Errorf("gemini: module %s twice in one summary request", m.Key)
		}
		seen[m.Key] = true
		inputs[i] = summaryInput{
			Key:        "m" + strconv.Itoa(i+1),
			TitleDE:    oneLine(m.TitleDE),
			TitleEN:    oneLine(m.TitleEN),
			Department: oneLine(m.Department),
			Contents:   clip(m.Contents, maxContentsChars),
			Outcomes:   clip(m.Outcomes, maxOutcomesChars),
		}
	}

	// Without HTML escaping, „<“ and „&“ reach the model as written, not as \u003c.
	var list bytes.Buffer
	enc := json.NewEncoder(&list)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(inputs); err != nil {
		return nil, fmt.Errorf("failed to encode modules: %w", err)
	}

	reqPayload := geminiRequest{
		Contents: []geminiContent{{Parts: []geminiPart{{Text: summaryPrompt + list.String()}}}},
		GenerationConfig: &geminiGenConfig{
			ResponseMimeType: "application/json",
			ResponseSchema:   json.RawMessage(summarySchema),
			Temperature:      0,
			MaxOutputTokens:  outputTokenMargin + summaryTokens*len(modules),
		},
	}
	jsonText, err := c.generate(ctx, reqPayload)
	var unusable *answerError
	if errors.As(err, &unusable) {
		return nil, allFailed(modules, unusable.Error())
	}
	if err != nil {
		return nil, err
	}
	var answers []summaryAnswer
	if err := json.Unmarshal([]byte(jsonText), &answers); err != nil {
		// Not the text itself: it may be long, and the error is logged every cycle.
		return nil, allFailed(modules, fmt.Sprintf("answer of %d bytes is not the JSON asked for: %v", len(jsonText), err))
	}
	return checkSummaries(modules, inputs, answers)
}

// allFailed names every module of a request whose answer as a whole is unusable.
func allFailed(modules []ModuleText, reason string) *IncompleteError {
	failed := &IncompleteError{Reasons: make(map[string]string, len(modules))}
	for _, m := range modules {
		failed.Keys = append(failed.Keys, m.Key)
		failed.Reasons[m.Key] = reason
	}
	return failed
}

// checkSummaries binds the answers to the modules by their keys of the request
// and keeps the valid ones. An answer for a key that was not asked for is ignored.
func checkSummaries(modules []ModuleText, inputs []summaryInput, answers []summaryAnswer) ([]ModuleSummary, error) {
	index := make(map[string]int, len(inputs))
	for i, in := range inputs {
		index[in.Key] = i
	}
	got := make([][]summaryAnswer, len(inputs))
	for _, a := range answers {
		if i, ok := index[strings.TrimSpace(a.Key)]; ok {
			got[i] = append(got[i], a)
		}
	}

	var out []ModuleSummary
	failed := &IncompleteError{Reasons: map[string]string{}}
	for i, m := range modules {
		var reason string
		var s ModuleSummary
		switch len(got[i]) {
		case 0:
			reason = "no answer"
		case 1:
			s, reason = checkSummary(got[i][0])
		default:
			reason = fmt.Sprintf("answered %d times", len(got[i]))
		}
		if reason != "" {
			failed.Keys = append(failed.Keys, m.Key)
			failed.Reasons[m.Key] = reason
			continue
		}
		s.Key = m.Key
		out = append(out, s)
	}
	if len(failed.Keys) > 0 {
		return out, failed
	}
	return out, nil
}

// checkSummary tidies one answer and says what is wrong with it, if anything.
// Keywords are trimmed, and empty, repeated and overlong ones dropped, before they
// are counted.
func checkSummary(a summaryAnswer) (ModuleSummary, string) {
	s := ModuleSummary{SummaryDE: oneLine(a.SummaryDE), SummaryEN: oneLine(a.SummaryEN)}
	for _, f := range []struct{ name, text string }{{"summary_de", s.SummaryDE}, {"summary_en", s.SummaryEN}} {
		if f.text == "" {
			return s, f.name + " empty"
		}
		if n := utf8.RuneCountInString(f.text); n > maxSummaryChars {
			return s, fmt.Sprintf("%s of %d characters", f.name, n)
		}
	}
	seen := map[string]bool{}
	for _, k := range a.Keywords {
		k = oneLine(k)
		if k == "" || seen[strings.ToLower(k)] || utf8.RuneCountInString(k) > maxKeywordChars {
			continue
		}
		seen[strings.ToLower(k)] = true
		s.Keywords = append(s.Keywords, k)
	}
	if n := len(s.Keywords); n < minKeywords || n > maxKeywords {
		return s, fmt.Sprintf("%d keywords", n)
	}
	return s, ""
}

// oneLine trims s and joins its runs of white space, line breaks included, into
// single spaces. Invisible format characters (soft hyphens, zero-width spaces, …)
// are dropped: a text of nothing else is empty.
func oneLine(s string) string {
	s = strings.Map(func(r rune) rune {
		if unicode.Is(unicode.Cf, r) {
			return -1
		}
		return r
	}, s)
	return strings.Join(strings.Fields(s), " ")
}

// clip makes s one line and cuts it to at most max characters at the end of a
// word, marking the cut with an ellipsis. A last word that reaches back beyond
// half of max is cut within.
func clip(s string, max int) string {
	s = oneLine(s)
	r := []rune(s)
	if len(r) <= max {
		return s
	}
	// r[max] is the first character left out: when it is a space, r[:max] ends
	// with a whole word.
	cut := max
	for i := max; i > max/2; i-- {
		if r[i] == ' ' {
			cut = i
			break
		}
	}
	return strings.TrimRight(string(r[:cut]), " ,;:-–") + " …"
}
