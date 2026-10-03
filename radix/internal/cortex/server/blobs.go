package server

import (
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// openBlob opens a blob for http.ServeContent: the file itself when it is stored raw, else a
// reader of the decompressed content that seeks by reading forward (and by opening the blob
// again to go back), so that Range works on a compressed blob too. size is the original size.
func (s *Server) openBlob(hash string, size int64) (io.ReadSeekCloser, error) {
	f, err := s.st.OpenRaw(hash)
	if err == nil {
		return f, nil
	}
	if !errors.Is(err, store.ErrCompressed) {
		return nil, err
	}
	rc, err := s.st.OpenBlob(hash)
	if err != nil {
		return nil, err
	}
	return &inflating{st: s.st, hash: hash, size: size, rc: rc, budget: inflateBudget(size)}, nil
}

// inflateBudget is how many bytes one reader of a compressed blob of size bytes may decompress:
// the content twice (http.ServeContent sniffs the start, then seeks back for the answer; a
// client may ask for a range near the start after one near the end) and some slack. Every
// backward seek decompresses from the start again, so without a bound a request of many
// alternating ranges costs a full decompression per range: minutes of CPU for one request.
// boundRanges keeps the honest requests far below it.
func inflateBudget(size int64) int64 { return 2*size + 1<<20 }

// errInflateBudget ends a read of a compressed blob that went back and forth too often.
var errInflateBudget = errors.New("blob: too many backward seeks in a compressed blob")

// inflating reads a compressed blob at any offset.
type inflating struct {
	st   *store.Store
	hash string
	size int64

	rc  io.ReadCloser // the decompressed content, at pos
	pos int64
	off int64 // where the next Read starts

	inflated int64 // bytes decompressed so far, over every reopening
	budget   int64 // the most inflated may reach
}

func (b *inflating) Seek(offset int64, whence int) (int64, error) {
	var abs int64
	switch whence {
	case io.SeekStart:
		abs = offset
	case io.SeekCurrent:
		abs = b.off + offset
	case io.SeekEnd:
		abs = b.size + offset
	default:
		return 0, errors.New("seek: invalid whence")
	}
	if abs < 0 {
		return 0, errors.New("seek: negative position")
	}
	b.off = abs
	return abs, nil
}

func (b *inflating) Read(p []byte) (int, error) {
	if b.off < b.pos {
		// Going back means decompressing b.off bytes again before the next byte is read.
		if b.inflated+b.off > b.budget {
			return 0, errInflateBudget
		}
		_ = b.rc.Close()
		rc, err := b.st.OpenBlob(b.hash)
		if err != nil {
			b.rc = io.NopCloser(strings.NewReader(""))
			return 0, err
		}
		b.rc, b.pos = rc, 0
	}
	if b.off > b.pos {
		n, err := io.CopyN(io.Discard, b.rc, b.off-b.pos)
		b.pos += n
		b.inflated += n
		if err != nil {
			return 0, err
		}
	}
	n, err := b.rc.Read(p)
	b.pos += int64(n)
	b.inflated += int64(n)
	b.off = b.pos
	return n, err
}

func (b *inflating) Close() error { return b.rc.Close() }

// maxRanges is the most ranges a request may ask for at once.
const maxRanges = 16

// boundRanges drops a Range header of more than two ranges unless they are at most maxRanges,
// in ascending order and apart, so that the full content is sent (200) instead, as RFC 9110
// §14.2 allows. net/http limits neither the count nor the order: on a compressed blob every
// range before the previous one decompresses the blob from the start again, so "0-0,-1"
// repeated thousands of times keeps a CPU busy for minutes (inflating's budget would cut such
// an answer off; this answers it whole instead). Two ranges in any order cost at most one
// backward seek, two decompressions, so they are kept. One range, ascending ranges and a header
// net/http itself refuses (malformed: 416) are left alone too.
func boundRanges(h http.Header) {
	spec, ok := strings.CutPrefix(h.Get("Range"), "bytes=")
	if !ok || !strings.Contains(spec, ",") {
		return
	}
	type span struct{ start, end int64 } // -1: absent (a suffix range has no start, "a-" no end)
	var spans []span
	for part := range strings.SplitSeq(spec, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		first, last, ok := strings.Cut(part, "-")
		first, last = strings.TrimSpace(first), strings.TrimSpace(last)
		sp := span{-1, -1}
		var err error
		switch {
		case !ok || first == "" && last == "":
			return // malformed: net/http answers it
		case first != "":
			if sp.start, err = parseDigits(first); err != nil {
				return
			}
			if last != "" {
				if sp.end, err = parseDigits(last); err != nil || sp.end < sp.start {
					return
				}
			}
		default:
			if _, err = parseDigits(last); err != nil {
				return
			}
		}
		spans = append(spans, sp)
	}
	if len(spans) <= 2 {
		return
	}
	if len(spans) > maxRanges {
		h.Del("Range")
		return
	}
	prevEnd := int64(-1)
	for i, sp := range spans {
		// A suffix range's place is known only from the size; nothing comes after "a-".
		if sp.start < 0 || sp.start <= prevEnd || sp.end < 0 && i < len(spans)-1 {
			h.Del("Range")
			return
		}
		prevEnd = sp.end
	}
}

// parseDigits reads a non-negative decimal number of ASCII digits only (no sign).
func parseDigits(s string) (int64, error) {
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("%q: not a number", s)
		}
	}
	return strconv.ParseInt(s, 10, 64)
}

// blobHash reads "sha256:<hex>" from the path.
func blobHash(r *http.Request) (string, bool) {
	hex, ok := strings.CutPrefix(r.PathValue("hash"), "sha256:")
	return hex, ok && store.ValidHash(hex)
}

// acceptsGzip says whether the client named gzip in Accept-Encoding (with a q above 0).
func acceptsGzip(r *http.Request) bool {
	for _, value := range r.Header.Values("Accept-Encoding") {
		for _, part := range strings.Split(value, ",") {
			coding, params, _ := strings.Cut(strings.TrimSpace(part), ";")
			if !strings.EqualFold(strings.TrimSpace(coding), "gzip") {
				continue
			}
			if q, ok := strings.CutPrefix(strings.ReplaceAll(params, " ", ""), "q="); ok {
				if v, err := strconv.ParseFloat(q, 64); err == nil && v <= 0 {
					continue
				}
			}
			return true
		}
	}
	return false
}

// handleGetBlob is GET and HEAD /v1/blobs/sha256:<hex>: the content by its hash, immutable.
// A client that accepts gzip gets a compressed blob as it is stored (Content-Encoding: gzip),
// unless it asks for a Range. A follower forwards a blob it lacks.
func (s *Server) handleGetBlob(w http.ResponseWriter, r *http.Request) {
	hash, ok := blobHash(r)
	if !ok {
		writeError(w, http.StatusBadRequest, codeBadRequest, "want /v1/blobs/sha256:<64 lower-case hex digits>")
		return
	}
	if !s.st.HasBlob(hash) {
		if !s.leading() && s.forward(w, r) {
			return
		}
		writeError(w, http.StatusNotFound, codeNotFound, "no blob sha256:"+hash)
		return
	}
	h := w.Header()
	etag := `"sha256:` + hash + `"`
	h.Set("ETag", etag)
	h.Set("Cache-Control", "public, max-age=31536000, immutable")
	h.Set("Vary", "Accept-Encoding")
	h.Set("Content-Type", "application/octet-stream")

	if acceptsGzip(r) && r.Header.Get("Range") == "" {
		rc, gz, stored, err := s.st.OpenStored(hash)
		if err != nil {
			writeInternal(w, "blob "+hash, err)
			return
		}
		defer rc.Close()
		if gz {
			if etagMatches(r.Header.Get("If-None-Match"), etag) {
				h.Del("Content-Type")
				w.WriteHeader(http.StatusNotModified)
				return
			}
			h.Set("Content-Encoding", "gzip")
			h.Set("Content-Length", strconv.FormatInt(stored, 10))
			w.WriteHeader(http.StatusOK)
			if r.Method != http.MethodHead {
				_, _ = io.Copy(w, rc)
			}
			return
		}
	}
	b, err := s.st.StatBlob(hash)
	if err != nil {
		writeInternal(w, "blob "+hash, err)
		return
	}
	if b.Size < 0 {
		s.streamBlob(w, r, hash, etag)
		return
	}
	body, err := s.openBlob(hash, b.Size)
	if err != nil {
		writeInternal(w, "blob "+hash, err)
		return
	}
	defer body.Close()
	http.ServeContent(w, r, "", time.Time{}, body)
}

// streamBlob sends the content of a blob whose original size the store does not know (a
// compressed blob no row names, of a length gzip's trailer cannot count exactly): without
// Content-Length, which would otherwise cut the body short; HEAD has none either. A Range is
// ignored, as RFC 9110 allows, since no part can be cut from an unknown length.
func (s *Server) streamBlob(w http.ResponseWriter, r *http.Request, hash, etag string) {
	if etagMatches(r.Header.Get("If-None-Match"), etag) {
		w.Header().Del("Content-Type")
		w.WriteHeader(http.StatusNotModified)
		return
	}
	if r.Method == http.MethodHead {
		w.WriteHeader(http.StatusOK)
		return
	}
	rc, err := s.st.OpenBlob(hash)
	if err != nil {
		writeInternal(w, "blob "+hash, err)
		return
	}
	defer rc.Close()
	w.WriteHeader(http.StatusOK)
	_, _ = io.Copy(w, rc)
}

// blobJSON describes a stored blob.
type blobJSON struct {
	SHA256 string `json:"sha256"` // sha256:<hex>
	Size   int64  `json:"size"`   // -1 when the store does not know it (StatBlob)
	Stored int64  `json:"stored"`
	Gzip   bool   `json:"gzip"`
}

func blobInfo(b store.BlobInfo) blobJSON {
	return blobJSON{SHA256: "sha256:" + b.Hash, Size: b.Size, Stored: b.Stored, Gzip: b.Gzip}
}

// handlePutBlob is PUT /v1/blobs/sha256:<hex>: an upload by hash, checked while it is
// written: 201, or 200 without reading the body when the blob is there; 422 hash-mismatch
// when the content has another hash. A follower forwards it.
//
// Log events: blob.rejected (WARN).
func (s *Server) handlePutBlob(w http.ResponseWriter, r *http.Request) {
	hash, ok := blobHash(r)
	if !ok {
		writeError(w, http.StatusBadRequest, codeBadRequest, "want /v1/blobs/sha256:<64 lower-case hex digits>")
		return
	}
	if !s.leading() && s.forward(w, r) {
		return
	}
	if s.st.HasBlob(hash) {
		b, err := s.st.StatBlob(hash)
		if err != nil {
			writeInternal(w, "blob "+hash, err)
			return
		}
		writeJSON(w, http.StatusOK, blobInfo(b))
		return
	}
	body := &bodyReader{r: r.Body}
	b, err := s.st.PutBlob(body, hash, 0)
	switch {
	case body.err != nil:
		writeError(w, http.StatusBadRequest, codeBadRequest, "reading the body: "+body.err.Error())
		return
	case errors.Is(err, store.ErrHashMismatch):
		oplog.For("cortex").Warn("upload refused", "event", "blob.rejected", "sha256", hash, oplog.Err(err))
		writeError(w, http.StatusUnprocessableEntity, codeHashMismatch, err.Error())
		return
	case err != nil:
		writeInternal(w, "storing the blob", err)
		return
	}
	writeJSON(w, http.StatusCreated, blobInfo(b))
}

// bodyReader remembers the first error reading a request body: the client's fault, unlike
// an error of the store.
type bodyReader struct {
	r   io.Reader
	err error
}

func (b *bodyReader) Read(p []byte) (int, error) {
	n, err := b.r.Read(p)
	if err != nil && err != io.EOF && b.err == nil {
		b.err = err
	}
	return n, err
}

// sha256Param reads an optional expect=sha256:<hex>.
func sha256Param(v string) (string, error) {
	if v == "" {
		return "", nil
	}
	hex, ok := strings.CutPrefix(v, "sha256:")
	if !ok || !store.ValidHash(hex) {
		return "", fmt.Errorf("expect %q: sha256:<64 lower-case hex digits>", v)
	}
	return hex, nil
}
