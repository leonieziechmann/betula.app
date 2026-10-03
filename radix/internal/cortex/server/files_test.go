package server

import (
	"bytes"
	"compress/gzip"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/cortex/client"
	"github.com/leonieziechmann/betula/radix/internal/cortex/cluster"
	"github.com/leonieziechmann/betula/radix/internal/cortex/store"
)

// putFile stores content as name and returns the answer and its FileInfo.
func putFile(t *testing.T, c *testCortex, name, content string, header ...string) (*http.Response, fileJSON) {
	t.Helper()
	resp, body := do(t, http.MethodPut, c.URL+"/v1/files/"+name, strings.NewReader(content), header...)
	var info fileJSON
	if resp.StatusCode == http.StatusOK || resp.StatusCode == http.StatusCreated {
		if err := json.Unmarshal([]byte(body), &info); err != nil {
			t.Fatalf("PUT %s: %s", name, body)
		}
	}
	return resp, info
}

func TestFilesKeepTheirVersions(t *testing.T) {
	c := newTestCortex(t)
	const name = "plans/2026/Prüfungsordnung%20B.Sc..txt" // escaped as a client sends it

	resp, v1 := putFile(t, c, name, "first", "Content-Type", "text/plain")
	if resp.StatusCode != http.StatusCreated || v1.Name != "plans/2026/Prüfungsordnung B.Sc..txt" || v1.SHA256 != "sha256:"+sha("first") ||
		v1.Size != 5 || v1.ContentType != "text/plain" || v1.CreatedAt != "2026-10-02T10:00:00.000000Z" || v1.Version == 0 {
		t.Fatalf("first PUT: %d %+v", resp.StatusCode, v1)
	}
	if resp, again := putFile(t, c, name, "first", "Content-Type", "text/plain"); resp.StatusCode != http.StatusOK || again.Version != v1.Version {
		t.Errorf("the same content again: %d %+v, want 200 and the same version", resp.StatusCode, again)
	}
	c.clock.advance(time.Hour)
	resp, v2 := putFile(t, c, name, "second")
	if resp.StatusCode != http.StatusCreated || v2.Version <= v1.Version || v2.ContentType != "application/octet-stream" {
		t.Errorf("second PUT: %d %+v", resp.StatusCode, v2)
	}

	resp, body := get(t, c.URL+"/v1/files/"+name)
	if resp.StatusCode != http.StatusOK || body != "second" || resp.Header.Get("ETag") != `"sha256:`+sha("second")+`"` ||
		resp.Header.Get("Last-Modified") != "Fri, 02 Oct 2026 11:00:00 GMT" || resp.Header.Get("Content-Type") != "application/octet-stream" {
		t.Errorf("GET: %d %q %v", resp.StatusCode, body, resp.Header)
	}
	if _, body := get(t, c.URL+"/v1/files/"+name+"?version="+itoa(v1.Version)); body != "first" {
		t.Errorf("?version=%d: %q", v1.Version, body)
	}
	if _, body := get(t, c.URL+"/v1/files/"+name+"?at="+url.QueryEscape(t0.Add(30*time.Minute).Format(time.RFC3339Nano))); body != "first" {
		t.Errorf("?at between: %q", body)
	}
	resp, body = get(t, c.URL+"/v1/files/"+name+"?at=2026-01-01T00:00:00Z")
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)

	resp, body = get(t, c.URL+"/v1/files/"+name, "If-None-Match", `"sha256:`+sha("second")+`"`)
	if resp.StatusCode != http.StatusNotModified {
		t.Errorf("If-None-Match: %d", resp.StatusCode)
	}
	resp, body = get(t, c.URL+"/v1/files/"+name, "Range", "bytes=1-3")
	if resp.StatusCode != http.StatusPartialContent || body != "eco" {
		t.Errorf("Range: %d %q", resp.StatusCode, body)
	}
	resp, body = do(t, http.MethodHead, c.URL+"/v1/files/"+name, nil)
	if resp.StatusCode != http.StatusOK || body != "" || resp.Header.Get("Content-Length") != "6" || resp.Header.Get("Cortex-Version") != itoa(v2.Version) {
		t.Errorf("HEAD: %d %v", resp.StatusCode, resp.Header)
	}

	// Deleted: gone for GET, its versions stay.
	if resp, body := do(t, http.MethodDelete, c.URL+"/v1/files/"+name, nil); resp.StatusCode != http.StatusNoContent {
		t.Fatalf("DELETE: %d %s", resp.StatusCode, body)
	}
	resp, body = get(t, c.URL+"/v1/files/"+name)
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	resp, body = do(t, http.MethodDelete, c.URL+"/v1/files/"+name, nil)
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	if _, body := get(t, c.URL+"/v1/files/"+name+"?version="+itoa(v1.Version)); body != "first" {
		t.Errorf("an old version after the delete: %q", body)
	}
	resp, body = get(t, c.URL+"/v1/files/"+name+"?versions")
	var list struct {
		Name     string     `json:"name"`
		Versions []fileJSON `json:"versions"`
	}
	if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &list) != nil || len(list.Versions) != 3 ||
		!list.Versions[0].Deleted || list.Versions[0].SHA256 != "" || list.Versions[2].SupersededAt != "2026-10-02T11:00:00.000000Z" {
		t.Errorf("?versions: %d %s", resp.StatusCode, body)
	}
}

func itoa(n int64) string { return strconv.FormatInt(n, 10) }

func TestIfMatchAndExpectGuardAPut(t *testing.T) {
	c := newTestCortex(t)
	resp, body := do(t, http.MethodPut, c.URL+"/v1/files/config.json", strings.NewReader("{}"), "If-Match", "*")
	wantStatus(t, resp, body, http.StatusPreconditionFailed, codePreconditionFailed)

	putFile(t, c, "config.json", "{}")
	resp, body = do(t, http.MethodPut, c.URL+"/v1/files/config.json", strings.NewReader(`{"a":1}`), "If-Match", `"sha256:`+sha("other")+`"`)
	wantStatus(t, resp, body, http.StatusPreconditionFailed, codePreconditionFailed)
	if resp, _ := putFile(t, c, "config.json", `{"a":1}`, "If-Match", `"sha256:`+sha("{}")+`"`); resp.StatusCode != http.StatusCreated {
		t.Errorf("If-Match with the current hash: %d", resp.StatusCode)
	}
	if resp, _ := putFile(t, c, "config.json", `{"a":2}`, "If-Match", "*"); resp.StatusCode != http.StatusCreated {
		t.Errorf("If-Match * on an existing file: %d", resp.StatusCode)
	}
	resp, body = do(t, http.MethodPut, c.URL+"/v1/files/config.json", strings.NewReader("x"), "If-Match", "sha256:unquoted")
	wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)

	resp, body = do(t, http.MethodPut, c.URL+"/v1/files/model.bin?expect=sha256:"+sha("weights"), strings.NewReader("other bytes"))
	wantStatus(t, resp, body, http.StatusUnprocessableEntity, codeHashMismatch)
	resp, body = get(t, c.URL+"/v1/files/model.bin")
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)
	if resp, _ := putFile(t, c, "model.bin?expect=sha256:"+sha("weights"), "weights"); resp.StatusCode != http.StatusCreated {
		t.Errorf("PUT with the expected hash: %d", resp.StatusCode)
	}
}

func TestFilesAreListedByPrefixAPageAtATime(t *testing.T) {
	c := newTestCortex(t)
	for _, name := range []string{"dir/a", "dir/b", "dir/c", "dir/d", "dir/e", "other"} {
		putFile(t, c, name, name)
	}
	do(t, http.MethodDelete, c.URL+"/v1/files/dir/c", nil)

	var names []string
	cursor := ""
	for pages := 0; ; pages++ {
		resp, body := get(t, c.URL+"/v1/files?prefix=dir/&limit=2&cursor="+url.QueryEscape(cursor))
		var out struct {
			Files      []fileJSON `json:"files"`
			NextCursor string     `json:"next_cursor"`
		}
		if resp.StatusCode != http.StatusOK || json.Unmarshal([]byte(body), &out) != nil || pages > 5 {
			t.Fatalf("listing: %d %s", resp.StatusCode, body)
		}
		for _, f := range out.Files {
			names = append(names, f.Name)
		}
		if cursor = out.NextCursor; cursor == "" {
			break
		}
	}
	if strings.Join(names, ",") != "dir/a,dir/b,dir/d,dir/e" {
		t.Errorf("listed %v", names)
	}
	resp, body := get(t, c.URL+"/v1/files?cursor=!!")
	wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)
}

func TestBadFileNamesAreRefused(t *testing.T) {
	c := newTestCortex(t)
	for _, path := range []string{
		"/v1/files/a//b", "/v1/files/a/../b", "/v1/files/./b", "/v1/files/a/", "/v1/files/", "/v1/files/a%2Fb",
		"/v1/files/a%00b", "/v1/files/" + strings.Repeat("x", 1025),
	} {
		for _, method := range []string{http.MethodPut, http.MethodGet} {
			resp, body := do(t, method, c.URL+path, strings.NewReader("x"))
			if resp.StatusCode != http.StatusBadRequest || resp.Header.Get("Cortex-Error") != codeBadName {
				t.Errorf("%s %s: %d %s", method, path, resp.StatusCode, body)
			}
		}
	}
	resp, body := do(t, http.MethodPost, c.URL+"/v1/files/a", strings.NewReader("x"))
	wantStatus(t, resp, body, http.StatusMethodNotAllowed, codeMethodNotAllowed)
}

func TestAContentTypeTheIndexCannotKeepIsABadRequest(t *testing.T) {
	c := newTestCortex(t)
	resp, body := do(t, http.MethodPut, c.URL+"/v1/files/a", strings.NewReader("x"), "Content-Type", "text/plain; name=\"M\xfcller\"")
	wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)
	if _, err := c.st.GetFile("a"); err == nil {
		t.Error("the file was stored")
	}
}

func TestTheClientPackageWorksAgainstTheServer(t *testing.T) {
	c := newTestCortex(t)
	cl, err := client.New(c.URL, client.Options{FailoverWait: -1})
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	info, created, err := cl.PutFile(ctx, "Studienpläne/B.Sc. Informatik.pdf", bytes.NewReader([]byte("%PDF-1.7")), client.PutOptions{ContentType: "application/pdf"})
	if err != nil || !created || info.SHA256 != "sha256:"+sha("%PDF-1.7") || info.ContentType != "application/pdf" {
		t.Fatalf("PutFile: %+v %v %v", info, created, err)
	}
	_, _, err = cl.PutFile(ctx, "Studienpläne/B.Sc. Informatik.pdf", strings.NewReader("x"), client.PutOptions{IfMatch: "sha256:" + sha("old")})
	if !errors.Is(err, client.ErrPreconditionFailed) {
		t.Errorf("PutFile with a stale If-Match: %v", err)
	}
	f, err := cl.GetFile(ctx, "Studienpläne/B.Sc. Informatik.pdf", client.GetFileOptions{})
	if err != nil {
		t.Fatal(err)
	}
	data, err := io.ReadAll(f.Body)
	f.Body.Close()
	if err != nil || string(data) != "%PDF-1.7" || f.Version != info.Version || f.SHA256 != info.SHA256 || f.ContentType != "application/pdf" {
		t.Errorf("GetFile: %+v %q %v", f.FileInfo, data, err)
	}
	if err := cl.DeleteFile(ctx, "Studienpläne/B.Sc. Informatik.pdf"); err != nil {
		t.Errorf("DeleteFile: %v", err)
	}
	if _, err := cl.GetFile(ctx, "Studienpläne/B.Sc. Informatik.pdf", client.GetFileOptions{}); !errors.Is(err, client.ErrNotFound) {
		t.Errorf("GetFile after the delete: %v", err)
	}
	st, err := cl.Status(ctx)
	if err != nil || st.Role != "leader" || st.Instance != "a" || st.Epoch != 1 || st.Follower == nil || st.Leader == nil || st.Version != "test" {
		t.Errorf("Status: %+v %v", st, err)
	}

	host := newFakeHost(t)
	host.set("/p", page{body: "through the transport"})
	hc := cl.HTTPClient(client.FetchOptions{Mode: client.ModeCache, MaxAge: -1, Stale: client.StaleNever}, 10*time.Second)
	resp, err := hc.Get(host.url("/p"))
	if err != nil {
		t.Fatal(err)
	}
	data, _ = io.ReadAll(resp.Body)
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK || string(data) != "through the transport" || client.CheckedAt(resp.Header) != t0 {
		t.Errorf("Transport: %d %q checked at %v", resp.StatusCode, data, client.CheckedAt(resp.Header))
	}
}

func TestBlobsByHash(t *testing.T) {
	c := newTestCortex(t)
	text := compressible(50_000)
	blobURL := c.URL + "/v1/blobs/sha256:" + sha(text)

	resp, body := do(t, http.MethodPut, blobURL, strings.NewReader(text[:100]))
	wantStatus(t, resp, body, http.StatusUnprocessableEntity, codeHashMismatch)
	resp, body = get(t, blobURL)
	wantStatus(t, resp, body, http.StatusNotFound, codeNotFound)

	resp, body = do(t, http.MethodPut, blobURL, strings.NewReader(text))
	var info blobJSON
	if resp.StatusCode != http.StatusCreated || json.Unmarshal([]byte(body), &info) != nil || info.SHA256 != "sha256:"+sha(text) ||
		info.Size != int64(len(text)) || !info.Gzip || info.Stored >= info.Size {
		t.Fatalf("PUT: %d %s", resp.StatusCode, body)
	}
	if resp, _ := do(t, http.MethodPut, blobURL, strings.NewReader("not even read")); resp.StatusCode != http.StatusOK {
		t.Errorf("PUT of a stored blob: %d, want 200", resp.StatusCode)
	}

	resp, body = get(t, blobURL)
	if resp.StatusCode != http.StatusOK || body != text || resp.Header.Get("Cache-Control") != "public, max-age=31536000, immutable" ||
		resp.Header.Get("ETag") != `"sha256:`+sha(text)+`"` || resp.Header.Get("Content-Encoding") != "" {
		t.Errorf("GET: %d %v", resp.StatusCode, resp.Header)
	}
	// gzip passed through as stored.
	resp, body = get(t, blobURL, "Accept-Encoding", "gzip")
	if resp.StatusCode != http.StatusOK || resp.Header.Get("Content-Encoding") != "gzip" || resp.Header.Get("Content-Length") != itoa(info.Stored) {
		t.Fatalf("GET gzip: %d %v", resp.StatusCode, resp.Header)
	}
	zr, err := gzip.NewReader(strings.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	if plain, err := io.ReadAll(zr); err != nil || string(plain) != text {
		t.Errorf("the gzip body does not inflate to the content: %v", err)
	}
	if resp, _ := get(t, blobURL, "Accept-Encoding", "gzip;q=0"); resp.Header.Get("Content-Encoding") != "" {
		t.Error("gzip;q=0 got gzip")
	}
	resp, body = get(t, blobURL, "Accept-Encoding", "gzip", "Range", "bytes=40000-40009")
	if resp.StatusCode != http.StatusPartialContent || body != text[40000:40010] || resp.Header.Get("Content-Encoding") != "" {
		t.Errorf("Range with gzip accepted: %d %q", resp.StatusCode, body)
	}
	resp, body = do(t, http.MethodHead, blobURL, nil)
	if resp.StatusCode != http.StatusOK || body != "" || resp.Header.Get("Content-Length") != itoa(int64(len(text))) {
		t.Errorf("HEAD: %d %v", resp.StatusCode, resp.Header)
	}
	if resp, _ := get(t, blobURL, "If-None-Match", `"sha256:`+sha(text)+`"`); resp.StatusCode != http.StatusNotModified {
		t.Errorf("If-None-Match: %d", resp.StatusCode)
	}

	// A raw blob: random bytes stay as they are.
	raw := randomText(3000, 7)
	rawURL := c.URL + "/v1/blobs/sha256:" + sha(raw)
	do(t, http.MethodPut, rawURL, strings.NewReader(raw))
	resp, body = get(t, rawURL, "Accept-Encoding", "gzip", "Range", "bytes=10-19")
	if resp.StatusCode != http.StatusPartialContent || body != raw[10:20] {
		t.Errorf("raw range: %d", resp.StatusCode)
	}
	resp, body = get(t, rawURL, "Accept-Encoding", "gzip")
	if resp.Header.Get("Content-Encoding") != "" || body != raw {
		t.Errorf("raw blob with gzip accepted: %v", resp.Header)
	}

	for _, bad := range []string{"/v1/blobs/abc", "/v1/blobs/sha256:XYZ", "/v1/blobs/md5:" + sha("x")} {
		resp, body := get(t, c.URL+bad)
		wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)
	}
}

// Review 2 delete-races-ifmatch-put: a DELETE takes the name's lock, which a conditional PUT
// holds from its If-Match check to its write, so the DELETE cannot land in between.
func TestADeleteWaitsForTheLockOfItsName(t *testing.T) {
	c := newTestCortex(t)
	putFile(t, c, "race.txt", "A")
	unlock := c.srv.files.lock("race.txt") // a PUT with If-Match between its check and its write
	deleted := make(chan int, 1)
	go func() {
		req, _ := http.NewRequest(http.MethodDelete, c.URL+"/v1/files/race.txt", nil)
		resp, err := http.DefaultTransport.RoundTrip(req)
		if err != nil {
			deleted <- 0
			return
		}
		resp.Body.Close()
		deleted <- resp.StatusCode
	}()
	select {
	case status := <-deleted:
		unlock()
		t.Fatalf("the DELETE answered %d while the name was locked", status)
	case <-time.After(300 * time.Millisecond):
	}
	unlock()
	select {
	case status := <-deleted:
		if status != http.StatusNoContent {
			t.Errorf("DELETE: %d", status)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the DELETE did not go on after the lock was released")
	}
}

// peeredNode is a leading node that knows the other instance of its pair (cluster.PeerNode).
type peeredNode struct {
	cluster.Node
	peer cluster.Info
}

func (n peeredNode) Peer() (cluster.Info, bool) { return n.peer, n.peer.URL != "" }

// removeBlobFile deletes a blob from st behind the store's back.
func removeBlobFile(t *testing.T, st *store.Store, hash string) {
	t.Helper()
	for _, suffix := range []string{"", ".gz"} {
		if err := os.Remove(filepath.Join(st.Dir(), "blobs", "sha256", hash[:2], hash+suffix)); err != nil && !errors.Is(err, os.ErrNotExist) {
			t.Fatal(err)
		}
	}
}

// E2E-4: a leader that lacks the blob of a file or a stored answer (promoted before its
// back-fill ended) reads it from the other instance, and answers 503 blob-missing, never 500,
// when that cannot serve it either.
func TestALeaderReadsABlobItLacksFromTheOtherInstance(t *testing.T) {
	other := newTestCortex(t) // the other instance, which has the blobs
	leader := newTestCortex(t, withNode(func(st *store.Store) cluster.Node {
		n, err := cluster.NewSingle(st, cluster.Info{Instance: "b", URL: "http://cortex-b.invalid"})
		if err != nil {
			t.Fatal(err)
		}
		return peeredNode{Node: n, peer: cluster.Info{Instance: "a", URL: other.URL}}
	}))
	content := "the plan of the semester"
	putFile(t, other, "plans/x.json", content)
	putFile(t, leader, "plans/x.json", content)
	removeBlobFile(t, leader.st, sha(content))
	resp, body := get(t, leader.URL+"/v1/files/plans/x.json")
	if resp.StatusCode != http.StatusOK || body != content {
		t.Errorf("a file whose blob the leader lacks: %d %q", resp.StatusCode, body)
	}

	target := "http://example.org/page"
	storeVersion(t, other.st, target, http.StatusOK, "stored page", t0)
	storeVersion(t, leader.st, target, http.StatusOK, "stored page", t0)
	removeBlobFile(t, leader.st, sha("stored page"))
	resp, body = get(t, leader.fetchURL(target, "mode", "offline"))
	if resp.StatusCode != http.StatusOK || body != "stored page" {
		t.Errorf("an offline fetch whose blob the leader lacks: %d %q", resp.StatusCode, body)
	}

	removeBlobFile(t, other.st, sha(content)) // nobody has it now
	resp, body = get(t, leader.URL+"/v1/files/plans/x.json")
	wantStatus(t, resp, body, http.StatusServiceUnavailable, codeBlobMissing)
	if resp.Header.Get("Retry-After") == "" {
		t.Error("503 blob-missing without Retry-After")
	}
}
