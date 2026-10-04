package server

import (
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/oplog"
	"github.com/leonieziechmann/betula/cortex/internal/store"
)

// fileJSON is a version of a file as the API shows it.
type fileJSON struct {
	Name         string `json:"name"`
	Version      int64  `json:"version"`
	SHA256       string `json:"sha256,omitempty"` // sha256:<hex>; none for a deletion
	Size         int64  `json:"size"`
	ContentType  string `json:"content_type,omitempty"`
	CreatedAt    string `json:"created_at"`
	Deleted      bool   `json:"deleted,omitempty"`
	SupersededAt string `json:"superseded_at,omitempty"`
}

func fileOf(fv store.FileVersion) fileJSON {
	f := fileJSON{Name: fv.Name, Version: fv.ID, Size: fv.Size, ContentType: fv.ContentType,
		CreatedAt: store.FormatTime(fv.CreatedAt), Deleted: fv.Deleted, SupersededAt: formatOptional(fv.SupersededAt)}
	if fv.Hash != "" {
		f.SHA256 = "sha256:" + fv.Hash
	}
	return f
}

const filesPrefix = "/v1/files/"

// fileName reads the name from the escaped path: each segment unescaped on its own, so that
// an escaped '/' cannot make a segment of its own; then the naming rules.
func fileName(r *http.Request) (string, bool) {
	escaped, ok := strings.CutPrefix(r.URL.EscapedPath(), filesPrefix)
	if !ok {
		return "", false
	}
	segments := strings.Split(escaped, "/")
	for i, seg := range segments {
		v, err := url.PathUnescape(seg)
		if err != nil || strings.Contains(v, "/") {
			return "", false
		}
		segments[i] = v
	}
	name := strings.Join(segments, "/")
	return name, store.ValidFileName(name)
}

// handleFile is /v1/files/{name...}: GET and HEAD read, PUT stores, DELETE deletes.
func (s *Server) handleFile(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet, http.MethodHead, http.MethodPut, http.MethodDelete:
	default:
		methodNotAllowed([]string{"GET", "HEAD", "PUT", "DELETE"})(w, r)
		return
	}
	name, ok := fileName(r)
	if !ok {
		writeError(w, http.StatusBadRequest, codeBadName,
			`a file name is 1 to 1024 bytes of UTF-8 in segments separated by "/", none of them empty, "." or "..", without control characters`)
		return
	}
	switch r.Method {
	case http.MethodPut:
		s.putFile(w, r, name)
	case http.MethodDelete:
		s.deleteFile(w, r, name)
	default:
		s.getFile(w, r, name)
	}
}

// ifMatch is a parsed If-Match: "*", or the hashes of the listed sha256 ETags (strong
// comparison: a weak tag never matches).
type ifMatch struct {
	any    bool
	hashes []string
}

func parseIfMatch(header string) (*ifMatch, error) {
	header = strings.TrimSpace(header)
	if header == "" {
		return nil, nil
	}
	if header == "*" {
		return &ifMatch{any: true}, nil
	}
	m := &ifMatch{}
	for _, part := range strings.Split(header, ",") {
		part = strings.TrimSpace(part)
		if strings.HasPrefix(part, "W/") {
			continue
		}
		tag, ok := strings.CutPrefix(part, `"`)
		if tag, ok = strings.CutSuffix(tag, `"`); !ok || tag == "" {
			return nil, fmt.Errorf(`If-Match %q: want "*" or "sha256:<hex>" in quotes`, header)
		}
		if hex, ok := strings.CutPrefix(tag, "sha256:"); ok {
			m.hashes = append(m.hashes, hex)
		}
	}
	return m, nil
}

// holds says whether the precondition holds for the current version (exists false: none).
func (m *ifMatch) holds(cur store.FileVersion, exists bool) bool {
	if m == nil {
		return true
	}
	if !exists {
		return false
	}
	if m.any {
		return true
	}
	for _, h := range m.hashes {
		if h == cur.Hash {
			return true
		}
	}
	return false
}

// precondition checks If-Match against the current version of name: false when it answered.
func (s *Server) precondition(w http.ResponseWriter, name string, m *ifMatch) bool {
	if m == nil {
		return true
	}
	cur, err := s.st.GetFile(name)
	if err != nil && !errors.Is(err, store.ErrNotFound) {
		writeInternal(w, "file "+name, err)
		return false
	}
	if !m.holds(cur, err == nil) {
		msg := "the file does not exist"
		if err == nil {
			msg = "the current version is sha256:" + cur.Hash
		}
		writeError(w, http.StatusPreconditionFailed, codePreconditionFailed, msg)
		return false
	}
	return true
}

// putFile is PUT /v1/files/{name}[?expect=sha256:…]: the body becomes the file's content, with
// its Content-Type (default application/octet-stream). 201 for a new version, 200 when the
// content and type are those of the current one; 412 precondition-failed when If-Match does
// not hold; 422 hash-mismatch when the content has another hash than expect. A follower
// forwards it.
//
// Log events: blob.rejected (WARN).
func (s *Server) putFile(w http.ResponseWriter, r *http.Request, name string) {
	expect, err := sha256Param(r.URL.Query().Get("expect"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	m, err := parseIfMatch(r.Header.Get("If-Match"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	if !s.leading() && s.forward(w, r) {
		return
	}
	// Checked before the upload, so that a refused one is not sent in vain, and again below.
	if !s.precondition(w, name, m) {
		return
	}
	contentType := r.Header.Get("Content-Type")
	if contentType == "" {
		contentType = "application/octet-stream"
	}

	body := &bodyReader{r: r.Body}
	blob, err := s.st.PutBlob(body, expect, 0)
	switch {
	case body.err != nil:
		writeError(w, http.StatusBadRequest, codeBadRequest, "reading the body: "+body.err.Error())
		return
	case errors.Is(err, store.ErrHashMismatch):
		oplog.For("cortex").Warn("upload refused", "event", "blob.rejected", "file", name, oplog.Err(err))
		writeError(w, http.StatusUnprocessableEntity, codeHashMismatch, err.Error())
		return
	case err != nil:
		writeInternal(w, "storing the content", err)
		return
	}

	unlock := s.files.lock(name)
	defer unlock()
	if !s.precondition(w, name, m) {
		return
	}
	done, ok := s.node.BeginWrite()
	if !ok { // the body is read: the client sends it again, to the leader
		s.noLeader(w, r, "this instance stopped leading during the upload")
		return
	}
	fv, created, _, err := s.st.PutFile(name, blob, contentType, s.now())
	done()
	switch {
	case errors.Is(err, store.ErrInvalidInput): // a Content-Type that is not UTF-8 or has a control character
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	case err != nil:
		writeInternal(w, "file "+name, err)
		return
	}
	w.Header().Set("ETag", `"sha256:`+fv.Hash+`"`)
	w.Header().Set("Cortex-Version", strconv.FormatInt(fv.ID, 10))
	status := http.StatusOK
	if created {
		status = http.StatusCreated
	}
	writeJSON(w, status, fileOf(fv))
}

// getFile is GET and HEAD /v1/files/{name}: the current content, the version current at a
// time (?at=RFC3339) or one version (?version=<id>), with ETag "sha256:<hex>", Last-Modified
// (when the version was stored), Cortex-Version, Range and If-None-Match; ?versions lists the
// versions instead. A follower that does not have it asks the leader; a leader that lacks its
// blob asks the other instance (503 blob-missing when it cannot answer either).
func (s *Server) getFile(w http.ResponseWriter, r *http.Request, name string) {
	q := r.URL.Query()
	if q.Has("versions") {
		versions, err := s.st.FileVersions(name)
		switch {
		case errors.Is(err, store.ErrNotFound):
			s.fileMissing(w, r, name)
			return
		case err != nil:
			writeInternal(w, "file "+name, err)
			return
		}
		out := struct {
			Name     string     `json:"name"`
			Versions []fileJSON `json:"versions"`
		}{Name: name, Versions: make([]fileJSON, 0, len(versions))}
		for _, fv := range versions {
			out.Versions = append(out.Versions, fileOf(fv))
		}
		writeJSON(w, http.StatusOK, out)
		return
	}

	var fv store.FileVersion
	var err error
	switch {
	case q.Get("version") != "":
		id, perr := strconv.ParseInt(q.Get("version"), 10, 64)
		if perr != nil || id <= 0 {
			writeError(w, http.StatusBadRequest, codeBadRequest, "version "+strconv.Quote(q.Get("version"))+": a version id")
			return
		}
		fv, err = s.st.GetFileVersion(name, id)
	case q.Get("at") != "":
		at, perr := time.Parse(time.RFC3339, q.Get("at"))
		if perr != nil {
			writeError(w, http.StatusBadRequest, codeBadRequest, "at "+strconv.Quote(q.Get("at"))+": not an RFC 3339 time")
			return
		}
		fv, err = s.st.GetFileAt(name, at)
	default:
		fv, err = s.st.GetFile(name)
	}
	switch {
	case errors.Is(err, store.ErrNotFound):
		s.fileMissing(w, r, name)
		return
	case err != nil:
		writeInternal(w, "file "+name, err)
		return
	}
	body, err := s.openBlob(fv.Hash, fv.Size)
	if err != nil {
		if errors.Is(err, store.ErrNotFound) { // not back-filled yet: the other instance has it (E2E-4)
			s.blobMissing(w, r, fv.Hash)
			return
		}
		writeInternal(w, "blob "+fv.Hash, err)
		return
	}
	defer body.Close()
	h := w.Header()
	h.Set("ETag", `"sha256:`+fv.Hash+`"`)
	h.Set("Cortex-Version", strconv.FormatInt(fv.ID, 10))
	h.Set("Content-Type", fv.ContentType)
	http.ServeContent(w, r, "", fv.CreatedAt, body)
}

// fileMissing answers a file this instance does not have: the leader may have it already.
func (s *Server) fileMissing(w http.ResponseWriter, r *http.Request, name string) {
	if !s.leading() && s.forward(w, r) {
		return
	}
	writeError(w, http.StatusNotFound, codeNotFound, "no file "+name)
}

// deleteFile is DELETE /v1/files/{name}: the current version becomes a deletion (204); the
// earlier ones stay readable by version and time until retention. 404 when there is no such
// file. A follower forwards it.
//
// It holds the name's lock as putFile does from its If-Match check to its write: a deletion
// between the two would be overwritten by a PUT whose condition no longer holds, and both
// would succeed (review 2: delete-races-ifmatch-put).
func (s *Server) deleteFile(w http.ResponseWriter, r *http.Request, name string) {
	unlock := s.files.lock(name)
	defer unlock()
	done := s.beginWrite(w, r)
	if done == nil {
		return
	}
	_, err := s.st.DeleteFile(name, s.now())
	done()
	switch {
	case errors.Is(err, store.ErrNotFound):
		writeError(w, http.StatusNotFound, codeNotFound, "no file "+name)
	case err != nil:
		writeInternal(w, "file "+name, err)
	default:
		w.WriteHeader(http.StatusNoContent)
	}
}

// handleListFiles is GET /v1/files?prefix=&cursor=&limit=: the current files (not deleted)
// whose names start with prefix, by name, a page of limit (100, at most 1000) at a time.
func (s *Server) handleListFiles(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query()
	limit, err := limitParam(q.Get("limit"))
	if err != nil {
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	}
	files, next, err := s.st.ListFiles(q.Get("prefix"), q.Get("cursor"), limit)
	switch {
	case errors.Is(err, store.ErrInvalidCursor):
		writeError(w, http.StatusBadRequest, codeBadRequest, err.Error())
		return
	case err != nil:
		writeInternal(w, "files", err)
		return
	}
	out := struct {
		Files      []fileJSON `json:"files"`
		NextCursor string     `json:"next_cursor"`
	}{Files: make([]fileJSON, 0, len(files)), NextCursor: next}
	for _, fv := range files {
		out.Files = append(out.Files, fileOf(fv))
	}
	writeJSON(w, http.StatusOK, out)
}
