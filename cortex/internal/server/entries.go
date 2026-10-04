package server

import (
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/store"
	"github.com/leonieziechmann/betula/cortex/internal/upstream"
)

// The JSON of the store's rows: hashes as sha256:<hex>, times in RFC 3339 with microseconds.

type entryJSON struct {
	ID             int64  `json:"id"`
	Key            string `json:"key"`
	URL            string `json:"url"`
	Host           string `json:"host"`
	Source         string `json:"source"`
	Accept         string `json:"accept,omitempty"`
	AcceptLanguage string `json:"accept_language,omitempty"`
	CreatedAt      string `json:"created_at"`
	CurrentVersion int64  `json:"current_version,omitempty"`
}

type versionJSON struct {
	ID           int64       `json:"id"`
	Status       int         `json:"status"`
	SHA256       string      `json:"sha256"`
	Size         int64       `json:"size"`
	Headers      http.Header `json:"headers"`
	FetchedAt    string      `json:"fetched_at"`
	CheckedAt    string      `json:"checked_at"`
	SupersededAt string      `json:"superseded_at,omitempty"`
}

type entryInfoJSON struct {
	entryJSON
	Current versionJSON `json:"current"`
}

func entryOf(e store.Entry) entryJSON {
	return entryJSON{ID: e.ID, Key: e.Key, URL: e.URL, Host: e.Host, Source: e.Source, Accept: e.Accept,
		AcceptLanguage: e.AcceptLanguage, CreatedAt: store.FormatTime(e.CreatedAt), CurrentVersion: e.CurrentVersion}
}

func versionOf(v store.Version) versionJSON {
	headers := v.Header
	if headers == nil {
		headers = http.Header{}
	}
	return versionJSON{ID: v.ID, Status: v.Status, SHA256: "sha256:" + v.Hash, Size: v.Size, Headers: headers,
		FetchedAt: store.FormatTime(v.FetchedAt), CheckedAt: store.FormatTime(v.CheckedAt), SupersededAt: formatOptional(v.SupersededAt)}
}

// formatOptional is store.FormatTime, "" for the zero time.
func formatOptional(t time.Time) string {
	if t.IsZero() {
		return ""
	}
	return store.FormatTime(t)
}

// limitParam reads limit: empty or 0 the default, at most store.MaxLimit.
func limitParam(v string) (int, error) {
	if v == "" {
		return store.DefaultLimit, nil
	}
	n, err := strconv.Atoi(v)
	if err != nil || n < 0 {
		return 0, errors.New("limit " + strconv.Quote(v) + ": a whole number of 0 or more")
	}
	if n == 0 {
		return store.DefaultLimit, nil
	}
	return min(n, store.MaxLimit), nil
}

// entryKey reads url, accept and accept_language into the store's key.
func entryKey(r *http.Request) (string, error) {
	q := r.URL.Query()
	key, _, _, err := store.Canonical(q.Get("url"), q.Get("accept"), q.Get("accept_language"))
	return key, err
}

// handleEntries is GET /v1/entries: with url (and accept, accept_language as the fetch had
// them), the versions of that request, newest first; else a listing of the entries with a
// current version, filtered by host, source and changed_since, a page of limit (100, at most
// 1000) at a time. Both roles answer from their own copy.
func (s *Server) handleEntries(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query()
	if q.Has("url") {
		key, err := entryKey(r)
		if err != nil {
			writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
			return
		}
		e, versions, err := s.st.Versions(key)
		switch {
		case errors.Is(err, store.ErrNotFound):
			writeError(w, http.StatusNotFound, codeNotFound, "no entry for "+q.Get("url"))
			return
		case err != nil:
			writeInternal(w, "versions", err)
			return
		}
		out := struct {
			Entry    entryJSON     `json:"entry"`
			Versions []versionJSON `json:"versions"`
		}{Entry: entryOf(e), Versions: make([]versionJSON, 0, len(versions))}
		for _, v := range versions {
			out.Versions = append(out.Versions, versionOf(v))
		}
		writeJSON(w, http.StatusOK, out)
		return
	}

	filter := store.EntryFilter{Host: strings.ToLower(q.Get("host")), Source: q.Get("source")}
	if v := q.Get("changed_since"); v != "" {
		t, err := time.Parse(time.RFC3339, v)
		if err != nil {
			writeError(w, http.StatusBadRequest, codeBadRequest, "changed_since "+strconv.Quote(v)+": not an RFC 3339 time")
			return
		}
		filter.ChangedSince = t
	}
	limit, err := limitParam(q.Get("limit"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	entries, next, err := s.st.ListEntries(filter, q.Get("cursor"), limit)
	switch {
	case errors.Is(err, store.ErrInvalidCursor):
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	case err != nil:
		writeInternal(w, "entries", err)
		return
	}
	out := struct {
		Entries    []entryInfoJSON `json:"entries"`
		NextCursor string          `json:"next_cursor"`
	}{Entries: make([]entryInfoJSON, 0, len(entries)), NextCursor: next}
	for _, e := range entries {
		out.Entries = append(out.Entries, entryInfoJSON{entryJSON: entryOf(e.Entry), Current: versionOf(e.Current)})
	}
	writeJSON(w, http.StatusOK, out)
}

// importParams is a request to PUT /v1/entries, checked.
type importParams struct {
	key, url, host         string // from store.Canonical
	accept, acceptLanguage string
	status                 int
	fetchedAt, checkedAt   time.Time
	source, expect         string
}

func parseImport(r *http.Request) (importParams, error) {
	q := r.URL.Query()
	p := importParams{source: defaultSource, accept: q.Get("accept"), acceptLanguage: q.Get("accept_language")}
	if v := q.Get("source"); v != "" {
		if !sourcePattern.MatchString(v) {
			return p, fmt.Errorf("source %q: 1 to 64 of a-z, 0-9, '_', '.', '-'", v)
		}
		p.source = v
	}
	if q.Get("url") == "" {
		return p, errors.New("url is required")
	}
	var err error
	if p.key, p.url, p.host, err = store.Canonical(q.Get("url"), p.accept, p.acceptLanguage); err != nil {
		return p, err
	}
	if p.status, err = strconv.Atoi(q.Get("status")); err != nil || !upstream.Storable(p.status) {
		return p, fmt.Errorf("status %q: one Cortex keeps (200, 203, 204, 404, 410)", q.Get("status"))
	}
	for _, t := range []struct {
		name string
		to   *time.Time
	}{{"fetched_at", &p.fetchedAt}, {"checked_at", &p.checkedAt}} {
		if *t.to, err = time.Parse(time.RFC3339, q.Get(t.name)); err != nil {
			return p, fmt.Errorf("%s %q: an RFC 3339 time is required", t.name, q.Get(t.name))
		}
	}
	if p.fetchedAt.After(p.checkedAt) {
		return p, fmt.Errorf("fetched_at %s is after checked_at %s", q.Get("fetched_at"), q.Get("checked_at"))
	}
	p.expect, err = sha256Param(q.Get("expect"))
	return p, err
}

// handleImport is PUT /v1/entries?url=…[&accept=…][&accept_language=…]&status=…&fetched_at=…
// &checked_at=…[&source=…][&expect=sha256:…]: an answer that another program fetched (an
// archive of its own, as Radix's raw pages) becomes what Cortex holds for that request, as if
// Cortex had fetched it first at fetched_at and last at checked_at (store.RecordImport). The
// body is the content, its Content-Type kept as the answer's. 201 with a new version; 200 when
// the current version has this content (result checked or unchanged) or the store has a newer
// answer (older); JSON {result, entry, version}. 422 hash-mismatch when the content has another
// hash than expect. A follower forwards it; the follower's copy follows from the journal.
//
// Log events: blob.rejected (WARN).
func (s *Server) handleImport(w http.ResponseWriter, r *http.Request) {
	p, err := parseImport(r)
	result := "error" // what the metric counts; nothing for a request the leader counts
	defer func() {
		if result != "" {
			importsTotal.Inc(p.source, result)
		}
	}()
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	info(r).url, info(r).source = p.url, p.source
	header := http.Header{}
	if v := r.Header.Get("Content-Type"); v != "" {
		header.Set("Content-Type", v)
	}
	if !s.leading() && s.forward(w, r) {
		result = ""
		return
	}

	body := &bodyReader{r: r.Body}
	blob, err := s.st.PutBlob(body, p.expect, 0)
	switch {
	case body.err != nil:
		writeError(w, http.StatusBadRequest, codeBadRequest, "reading the body: "+body.err.Error())
		return
	case errors.Is(err, store.ErrHashMismatch):
		oplog.For("cortex").Warn("import refused", "event", "blob.rejected", "url", p.url, oplog.Err(err))
		writeError(w, http.StatusUnprocessableEntity, codeHashMismatch, err.Error())
		return
	case err != nil:
		writeInternal(w, "storing the content", err)
		return
	}
	done, ok := s.node.BeginWrite()
	if !ok { // the body is read: the client sends it again, to the leader
		s.noLeader(w, r, "this instance stopped leading during the upload")
		return
	}
	e, v, outcome, err := s.st.RecordImport(store.Imported{Key: p.key, URL: p.url, Host: p.host, Source: p.source,
		Accept: p.accept, AcceptLanguage: p.acceptLanguage, Status: p.status, Hash: blob.Hash, Size: blob.Size,
		Header: header, FetchedAt: p.fetchedAt, CheckedAt: p.checkedAt, At: s.now()})
	done()
	switch {
	case errors.Is(err, store.ErrInvalidInput): // a checked_at in the future, a Content-Type that is not UTF-8
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	case err != nil:
		writeInternal(w, "import of "+p.url, err)
		return
	}
	result = outcome
	w.Header().Set("ETag", `"sha256:`+v.Hash+`"`)
	w.Header().Set("Cortex-Version", strconv.FormatInt(v.ID, 10))
	status := http.StatusOK
	if outcome == store.ImportCreated {
		status = http.StatusCreated
	}
	writeJSON(w, status, struct {
		Result  string      `json:"result"`
		Entry   entryJSON   `json:"entry"`
		Version versionJSON `json:"version"`
	}{outcome, entryOf(e), versionOf(v)})
}

// handleDeleteEntry is DELETE /v1/entries?url=…: the entry and its versions are forgotten
// (204; 404 when there is none). A follower forwards it.
func (s *Server) handleDeleteEntry(w http.ResponseWriter, r *http.Request) {
	key, err := entryKey(r)
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	done := s.beginWrite(w, r)
	if done == nil {
		return
	}
	_, err = s.st.DeleteEntry(key)
	done()
	switch {
	case errors.Is(err, store.ErrNotFound):
		writeError(w, http.StatusNotFound, codeNotFound, "no entry for "+r.URL.Query().Get("url"))
	case err != nil:
		writeInternal(w, "delete", err)
	default:
		w.WriteHeader(http.StatusNoContent)
	}
}
