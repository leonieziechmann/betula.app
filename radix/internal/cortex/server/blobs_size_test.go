package server

import (
	"bytes"
	"compress/gzip"
	"encoding/json"
	"net/http"
	"strings"
	"testing"
)

// Review 1 (statblob-isize-mod-4gib): the original size of a compressed blob that no row names
// was gzip's trailer, modulo 4 GiB, and went out as Content-Length, which cut a body of more
// than 4 GiB short without an error. When the store does not know the size (Size -1), the
// content is streamed without Content-Length, HEAD has none, and a Range is ignored.
func TestABlobOfUnknownSizeIsStreamedWithoutContentLength(t *testing.T) {
	c := newTestCortex(t)
	content := randomText(5<<20, 11)
	var stored bytes.Buffer
	zw, _ := gzip.NewWriterLevel(&stored, gzip.BestSpeed)
	_, _ = zw.Write([]byte(content))
	_ = zw.Close()
	// Uploaded by hash elsewhere, imported as it was stored there; no row names it.
	if err := c.st.ImportStored(sha(content), bytes.NewReader(stored.Bytes()), true); err != nil {
		t.Fatal(err)
	}
	if b, err := c.st.StatBlob(sha(content)); err != nil || b.Size != -1 {
		t.Fatalf("StatBlob = %+v (err %v), want size -1: the test does not test the unknown size", b, err)
	}
	blobURL := c.URL + "/v1/blobs/sha256:" + sha(content)
	etag := `"sha256:` + sha(content) + `"`

	resp, body := get(t, blobURL)
	if resp.StatusCode != http.StatusOK || body != content || resp.ContentLength != -1 ||
		resp.Header.Get("Content-Length") != "" || resp.Header.Get("ETag") != etag || resp.Header.Get("Content-Encoding") != "" {
		t.Errorf("GET: %d, %d bytes, Content-Length %d %v", resp.StatusCode, len(body), resp.ContentLength, resp.Header)
	}
	resp, body = do(t, http.MethodHead, blobURL, nil)
	if resp.StatusCode != http.StatusOK || body != "" || resp.Header.Get("Content-Length") != "" || resp.Header.Get("ETag") != etag {
		t.Errorf("HEAD: %d %v", resp.StatusCode, resp.Header)
	}
	// A part cannot be cut from an unknown length: the whole content (RFC 9110 lets a server
	// ignore Range).
	resp, body = get(t, blobURL, "Range", "bytes=10-19")
	if resp.StatusCode != http.StatusOK || body != content || resp.Header.Get("Content-Range") != "" {
		t.Errorf("Range: %d, %d bytes, Content-Range %q", resp.StatusCode, len(body), resp.Header.Get("Content-Range"))
	}
	if resp, body := get(t, blobURL, "If-None-Match", etag); resp.StatusCode != http.StatusNotModified || body != "" {
		t.Errorf("If-None-Match: %d", resp.StatusCode)
	}
	// A client that takes gzip gets the stored bytes, whose length is known.
	resp, body = get(t, blobURL, "Accept-Encoding", "gzip")
	if resp.StatusCode != http.StatusOK || body != stored.String() || resp.Header.Get("Content-Encoding") != "gzip" ||
		resp.Header.Get("Content-Length") != itoa(int64(stored.Len())) {
		t.Errorf("GET gzip: %d %v", resp.StatusCode, resp.Header)
	}
	// An upload of a blob that is there answers with what the store knows.
	resp, body = do(t, http.MethodPut, blobURL, strings.NewReader("not read"))
	var info blobJSON
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &info) != nil || info.Size != -1 ||
		info.Stored != int64(stored.Len()) || !info.Gzip {
		t.Errorf("PUT of a stored blob: %d %s", resp.StatusCode, body)
	}
}
