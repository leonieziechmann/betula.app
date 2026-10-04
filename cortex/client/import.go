package client

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"time"
)

// ImportOptions describe an answer another program fetched, for Import: what the request
// named that Cortex stores an answer under, and the answer.
type ImportOptions struct {
	Status    int       // the host's status: 200, 203, 204, 404 or 410
	FetchedAt time.Time // when the program first got this content
	CheckedAt time.Time // when it last got it
	Source    string    // what the request was for, as in FetchOptions; "": "unknown"

	// Part of what Cortex stores the answer under, as the headers of a fetch are: an answer
	// imported without them is served to a fetch without them.
	Accept, AcceptLanguage string

	ContentType string // the answer's Content-Type; "": none
	Expect      string // sha256:<hex>: Cortex refuses content with another hash
}

// What Cortex made of an import (ImportResult.Result).
const (
	ImportCreated   = "created"   // a new version, current now
	ImportChecked   = "checked"   // the current version had this content; it counts as checked when the program last got it
	ImportUnchanged = "unchanged" // the current version had this content and was checked as late already
	ImportOlder     = "older"     // Cortex has a newer answer of other content; nothing was stored
)

// ImportResult is Cortex's answer to an import.
type ImportResult struct {
	Result  string // ImportCreated, ImportChecked, ImportUnchanged or ImportOlder
	Version int64  // the current version afterwards
	SHA256  string // sha256:<hex> of its content
}

// Import gives Cortex an answer that another program fetched from rawURL (PUT /v1/entries,
// docs/cortex/cortex.md §4.3): body is its content, and Cortex serves it as if it had fetched
// it itself first at o.FetchedAt and last at o.CheckedAt, offline included. Cortex keeps a
// newer answer of its own (ImportOlder). An import changes nothing when it is sent again, so a
// caller may repeat one that failed with ErrOutcomeUnknown. The body is sent to the next
// instance on a fail-over as PutFile's is.
func (c *Client) Import(ctx context.Context, rawURL string, body io.Reader, o ImportOptions) (ImportResult, error) {
	if err := checkFetchURL(rawURL); err != nil {
		return ImportResult{}, err
	}
	if o.FetchedAt.IsZero() || o.CheckedAt.IsZero() {
		return ImportResult{}, fmt.Errorf("cortex: import of %s: FetchedAt and CheckedAt are required", rawURL)
	}
	q := url.Values{
		"url":        {rawURL},
		"status":     {strconv.Itoa(o.Status)},
		"fetched_at": {o.FetchedAt.UTC().Format(time.RFC3339Nano)},
		"checked_at": {o.CheckedAt.UTC().Format(time.RFC3339Nano)},
	}
	for name, value := range map[string]string{"source": o.Source, "accept": o.Accept, "accept_language": o.AcceptLanguage, "expect": o.Expect} {
		if value != "" {
			q.Set(name, value)
		}
	}
	r := request{method: http.MethodPut, target: "/v1/entries?" + q.Encode(), header: make(http.Header)}
	if o.ContentType != "" {
		r.header.Set("Content-Type", o.ContentType)
	}
	var err error
	if r.body, r.once, err = bodyOf(body); err != nil {
		return ImportResult{}, err
	}

	resp, err := c.do(ctx, r)
	if err != nil {
		return ImportResult{}, err
	}
	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		return ImportResult{}, errorFrom(resp)
	}
	defer resp.Body.Close()
	var out struct {
		Result  string `json:"result"`
		Version struct {
			ID     int64  `json:"id"`
			SHA256 string `json:"sha256"`
		} `json:"version"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, maxErrorBody)).Decode(&out); err != nil {
		return ImportResult{}, fmt.Errorf("cortex: the answer to the import of %s: %w", rawURL, err)
	}
	return ImportResult{Result: out.Result, Version: out.Version.ID, SHA256: out.Version.SHA256}, nil
}
