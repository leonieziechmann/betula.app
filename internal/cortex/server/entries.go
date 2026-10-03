package server

import (
	"errors"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/cortex/store"
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
