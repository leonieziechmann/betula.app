package client

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// fileStore is a fake Cortex that stores the files it is given, as the API describes.
type fileStore struct {
	mu    sync.Mutex
	files map[string]storedFile
	puts  []*http.Request
}

type storedFile struct {
	data        []byte
	contentType string
	version     int64
	deleted     bool
}

func (s *fileStore) serve(w http.ResponseWriter, r *http.Request) {
	s.mu.Lock()
	defer s.mu.Unlock()
	fail := func(status int, code string) {
		w.Header().Set("Cortex-Error", code)
		w.WriteHeader(status)
		fmt.Fprintf(w, `{"error":%q,"message":"%s %s"}`, code, code, r.URL.Path)
	}
	name, ok := strings.CutPrefix(r.URL.Path, "/v1/files/")
	if !ok {
		fail(http.StatusNotFound, "not-found")
		return
	}
	f, exists := s.files[name]
	sum := func(data []byte) string {
		h := sha256.Sum256(data)
		return "sha256:" + hex.EncodeToString(h[:])
	}
	switch r.Method {
	case http.MethodPut:
		data, err := io.ReadAll(r.Body)
		if err != nil {
			fail(http.StatusBadRequest, "bad-request")
			return
		}
		s.puts = append(s.puts, r.Clone(context.Background()))
		if m := r.Header.Get("If-Match"); m != "" && (!exists || f.deleted || (m != "*" && m != `"`+sum(f.data)+`"`)) {
			fail(http.StatusPreconditionFailed, "precondition-failed")
			return
		}
		if e := r.URL.Query().Get("expect"); e != "" && e != sum(data) {
			fail(http.StatusUnprocessableEntity, "hash-mismatch")
			return
		}
		contentType := r.Header.Get("Content-Type")
		if contentType == "" {
			contentType = "application/octet-stream"
		}
		status := http.StatusOK
		if !exists || f.deleted || !bytes.Equal(f.data, data) || f.contentType != contentType {
			f = storedFile{data: data, contentType: contentType, version: f.version + 1}
			s.files[name] = f
			status = http.StatusCreated
		}
		w.WriteHeader(status)
		_ = json.NewEncoder(w).Encode(map[string]any{"name": name, "version": f.version, "sha256": sum(f.data), "size": len(f.data),
			"content_type": f.contentType, "created_at": "2026-10-02T08:00:00.000000Z"})
	case http.MethodGet:
		if !exists || f.deleted {
			fail(http.StatusNotFound, "not-found")
			return
		}
		w.Header().Set("ETag", `"`+sum(f.data)+`"`)
		w.Header().Set("Cortex-Version", fmt.Sprint(f.version))
		w.Header().Set("Content-Type", f.contentType)
		w.Header().Set("Last-Modified", "Fri, 02 Oct 2026 08:00:00 GMT")
		if r.URL.Query().Get("corrupt") != "" || name == "corrupt" {
			_, _ = w.Write(append([]byte("x"), f.data...))
			return
		}
		_, _ = w.Write(f.data)
	case http.MethodDelete:
		if !exists || f.deleted {
			fail(http.StatusNotFound, "not-found")
			return
		}
		f.deleted = true
		s.files[name] = f
		w.WriteHeader(http.StatusNoContent)
	}
}

// received returns the PUT requests so far.
func (s *fileStore) received() []*http.Request {
	s.mu.Lock()
	defer s.mu.Unlock()
	return append([]*http.Request(nil), s.puts...)
}

func (s *fileStore) content(name string) string {
	s.mu.Lock()
	defer s.mu.Unlock()
	return string(s.files[name].data)
}

func newFileStore(t *testing.T) (*fileStore, *instance) {
	t.Helper()
	s := &fileStore{files: make(map[string]storedFile)}
	return s, newInstance(t, s.serve)
}

// readingNoLeader reads the request and answers that it knows no leader, as a follower does
// that waited for one in vain.
func readingNoLeader(w http.ResponseWriter, r *http.Request) {
	_, _ = io.Copy(io.Discard, r.Body)
	noLeaderAnswer(w, r)
}

// A body that can be read again goes to the next instance whole when the first one read it
// and answered no-leader; from where the reader stood, not from the start.
func TestPutFileSendsTheBodyAgainOnFailover(t *testing.T) {
	file := filepath.Join(t.TempDir(), "plan.json")
	if err := os.WriteFile(file, []byte(`skipped{"plan":1}`), 0o644); err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name string
		body func(t *testing.T) io.Reader
	}{
		{"file", func(t *testing.T) io.Reader {
			f, err := os.Open(file)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(func() { f.Close() })
			if _, err := f.Seek(int64(len("skipped")), io.SeekStart); err != nil {
				t.Fatal(err)
			}
			return f
		}},
		{"bytes.Reader", func(*testing.T) io.Reader { return bytes.NewReader([]byte(`{"plan":1}`)) }},
		{"strings.Reader", func(*testing.T) io.Reader { return strings.NewReader(`{"plan":1}`) }},
		{"bytes.Buffer", func(*testing.T) io.Reader { return bytes.NewBufferString(`{"plan":1}`) }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			a := newInstance(t, readingNoLeader)
			store, b := newFileStore(t)
			c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)

			want := sha256.Sum256([]byte(`{"plan":1}`))
			info, created, err := c.PutFile(context.Background(), "radix/plans/079 Ü.json", tc.body(t),
				PutOptions{ContentType: "application/json", Expect: "sha256:" + hex.EncodeToString(want[:])})
			if err != nil || !created {
				t.Fatalf("PutFile = %+v, %v, %v", info, created, err)
			}
			puts := store.received()
			if a.hits.Load() != 1 || len(puts) != 1 {
				t.Fatalf("hits a=%d, puts at b=%d", a.hits.Load(), len(puts))
			}
			put := puts[0]
			if put.URL.Path != "/v1/files/radix/plans/079 Ü.json" || put.URL.EscapedPath() != "/v1/files/radix/plans/079%20%C3%9C.json" ||
				put.ContentLength != int64(len(`{"plan":1}`)) || put.Header.Get("Content-Type") != "application/json" {
				t.Errorf("b got PUT %s (%s), length %d, type %q", put.URL.Path, put.URL.EscapedPath(), put.ContentLength, put.Header.Get("Content-Type"))
			}
			if got := store.content("radix/plans/079 Ü.json"); got != `{"plan":1}` {
				t.Errorf("stored %q", got)
			}
			if info.Name != "radix/plans/079 Ü.json" || info.Version != 1 || info.SHA256 != "sha256:"+hex.EncodeToString(want[:]) ||
				info.Size != 10 || info.ContentType != "application/json" || info.CreatedAt.IsZero() {
				t.Errorf("FileInfo = %+v", info)
			}
		})
	}
}

// committingThenDropping lets serve carry a request out and then resets the connection
// instead of answering, as an instance does that crashes right after its commit.
func committingThenDropping(t *testing.T, serve http.HandlerFunc) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		serve(httptest.NewRecorder(), r)
		dropping(t, true)(w, r)
	}
}

func sumOf(content string) string {
	h := sha256.Sum256([]byte(content))
	return "sha256:" + hex.EncodeToString(h[:])
}

// A write that an instance carried out before the connection broke is not sent to the
// other instance, which shares its files: there the If-Match would fail (412), the delete
// find nothing (404) and the new file be "not created", although this very call did each of
// them. The caller learns that the outcome is unknown, and the files show what happened.
func TestAWriteWhoseConnectionBrokeIsNotSentAgain(t *testing.T) {
	store := &fileStore{files: map[string]storedFile{
		"plan.json": {data: []byte("v1"), contentType: "application/octet-stream", version: 1},
		"old.txt":   {data: []byte("old"), contentType: "text/plain", version: 1},
	}}
	a := newInstance(t, committingThenDropping(t, store.serve))
	b := newInstance(t, store.serve)
	c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)
	ctx := context.Background()

	for _, tc := range []struct {
		name string
		call func() error
		done func() bool // whether a carried the write out
	}{
		{"PUT with If-Match", func() error {
			_, _, err := c.PutFile(ctx, "plan.json", strings.NewReader("v2"), PutOptions{IfMatch: sumOf("v1")})
			return err
		}, func() bool { return store.content("plan.json") == "v2" }},
		{"PUT of a new file", func() error {
			_, _, err := c.PutFile(ctx, "new.bin", bytes.NewReader([]byte("hello")), PutOptions{})
			return err
		}, func() bool { return store.content("new.bin") == "hello" }},
		{"DELETE", func() error { return c.DeleteFile(ctx, "old.txt") }, func() bool {
			store.mu.Lock()
			defer store.mu.Unlock()
			return store.files["old.txt"].deleted
		}},
		{"POST", func() error { return c.StepDown(ctx) }, func() bool { return true }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			aHits := a.hits.Load()
			err := tc.call()
			if !errors.Is(err, ErrOutcomeUnknown) || errors.Is(err, ErrPreconditionFailed) || errors.Is(err, ErrNotFound) {
				t.Errorf("err = %v, want ErrOutcomeUnknown", err)
			}
			if !tc.done() {
				t.Error("a did not carry the write out")
			}
			if a.hits.Load() != aHits+1 || b.hits.Load() != 0 {
				t.Errorf("a got %d requests, b %d; want the write sent once, to a", a.hits.Load()-aHits, b.hits.Load())
			}
		})
	}
	if puts := store.received(); len(puts) != 2 {
		t.Errorf("%d PUTs reached the files, want 2", len(puts))
	}
}

// Go's transport sends a request again by itself, on a new connection, when one it had used
// before broke before the answer, if it takes the request for idempotent: an Idempotency-Key
// (even an empty one, which is not sent) makes it take a PUT or DELETE so. With a single
// instance and a single round, nothing but the transport could send it twice.
func TestTheTransportDoesNotSendAWriteAgain(t *testing.T) {
	store := &fileStore{files: map[string]storedFile{"a.txt": {data: []byte("a"), contentType: "text/plain", version: 1}}}
	writes := map[string]*atomic.Int32{http.MethodPut: {}, http.MethodDelete: {}}
	crash := committingThenDropping(t, store.serve)
	in := newInstance(t, func(w http.ResponseWriter, r *http.Request) {
		if n, ok := writes[r.Method]; ok {
			n.Add(1)
			crash(w, r)
			return
		}
		store.serve(w, r)
	})
	c := newTestClient(t, -1, in.srv.URL)
	ctx := context.Background()
	// A request read to its end leaves its connection for the next one.
	keepAlive := func() {
		t.Helper()
		f, err := c.GetFile(ctx, "a.txt", GetFileOptions{})
		if err != nil {
			t.Fatalf("GetFile: %v", err)
		}
		if _, err := io.ReadAll(f.Body); err != nil {
			t.Fatalf("GetFile: %v", err)
		}
		f.Body.Close()
	}

	keepAlive()
	if _, _, err := c.PutFile(ctx, "b.txt", strings.NewReader("b"), PutOptions{}); !errors.Is(err, ErrOutcomeUnknown) {
		t.Errorf("PutFile: %v, want ErrOutcomeUnknown", err)
	}
	keepAlive()
	if err := c.DeleteFile(ctx, "a.txt"); !errors.Is(err, ErrOutcomeUnknown) {
		t.Errorf("DeleteFile: %v, want ErrOutcomeUnknown", err)
	}
	for method, n := range writes {
		if n.Load() != 1 {
			t.Errorf("the %s was sent %d times, want once", method, n.Load())
		}
	}
}

// A write goes to the next instance when the first cannot have carried it out: it could not
// be connected to, or it answered no-leader.
func TestAWriteGoesToTheNextInstanceWhenTheFirstDidNothing(t *testing.T) {
	for _, tc := range []struct {
		name  string
		first func(t *testing.T) string
	}{
		{"unreachable", refusing},
		{"no leader", func(t *testing.T) string { return newInstance(t, readingNoLeader).srv.URL }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			store, b := newFileStore(t)
			c := newTestClient(t, time.Second, tc.first(t), b.srv.URL)
			ctx := context.Background()
			if _, created, err := c.PutFile(ctx, "x", strings.NewReader("content"), PutOptions{}); err != nil || !created || store.content("x") != "content" {
				t.Errorf("PutFile = %v, %v; b stored %q", created, err, store.content("x"))
			}
			c = newTestClient(t, time.Second, tc.first(t), b.srv.URL)
			if err := c.DeleteFile(ctx, "x"); err != nil {
				t.Errorf("DeleteFile: %v", err)
			}
			if _, err := c.GetFile(ctx, "x", GetFileOptions{}); !errors.Is(err, ErrNotFound) {
				t.Errorf("GetFile after the delete: %v, want ErrNotFound", err)
			}
		})
	}
}

// A body that can be read once, from a pipe, goes to one instance only.
func TestPutFileFromAStreamIsSentOnce(t *testing.T) {
	a := newInstance(t, dropping(t, false))
	store, b := newFileStore(t)
	c := newTestClient(t, time.Second, a.srv.URL, b.srv.URL)

	pr, pw := io.Pipe()
	go func() {
		_, _ = io.WriteString(pw, "streamed")
		_ = pw.Close()
	}()
	if _, _, err := c.PutFile(context.Background(), "stream", pr, PutOptions{}); err == nil {
		t.Error("PutFile through a dropping instance succeeded")
	}
	if puts := store.received(); len(puts) != 0 {
		t.Errorf("the stream was sent to b as well: %d puts", len(puts))
	}

	// To an instance that takes it, a stream is stored whole.
	c = newTestClient(t, time.Second, b.srv.URL)
	pr, pw = io.Pipe()
	go func() {
		_, _ = io.WriteString(pw, "streamed")
		_ = pw.Close()
	}()
	if info, created, err := c.PutFile(context.Background(), "stream", pr, PutOptions{}); err != nil || !created || info.Size != 8 ||
		info.ContentType != "application/octet-stream" {
		t.Errorf("PutFile = %+v, %v, %v", info, created, err)
	}
}

func TestFileRoundTrip(t *testing.T) {
	_, b := newFileStore(t)
	c := newTestClient(t, time.Second, b.srv.URL)
	ctx := context.Background()

	if _, created, err := c.PutFile(ctx, "notes.txt", strings.NewReader("one"), PutOptions{ContentType: "text/plain"}); err != nil || !created {
		t.Fatalf("PutFile: %v, %v", created, err)
	}
	if _, created, err := c.PutFile(ctx, "notes.txt", strings.NewReader("one"), PutOptions{ContentType: "text/plain"}); err != nil || created {
		t.Errorf("the same content again: created=%v, %v; want unchanged", created, err)
	}
	if _, _, err := c.PutFile(ctx, "notes.txt", strings.NewReader("two"), PutOptions{IfMatch: "sha256:0000"}); !errors.Is(err, ErrPreconditionFailed) {
		t.Errorf("If-Match of another version: %v, want ErrPreconditionFailed", err)
	}
	one := sha256.Sum256([]byte("one"))
	if info, created, err := c.PutFile(ctx, "notes.txt", strings.NewReader("two"), PutOptions{ContentType: "text/plain",
		IfMatch: "sha256:" + hex.EncodeToString(one[:])}); err != nil || !created || info.Version != 2 {
		t.Errorf("If-Match of the current version: %+v, %v, %v", info, created, err)
	}

	f, err := c.GetFile(ctx, "notes.txt", GetFileOptions{})
	if err != nil {
		t.Fatalf("GetFile: %v", err)
	}
	data, err := io.ReadAll(f.Body)
	f.Body.Close()
	two := sha256.Sum256([]byte("two"))
	if err != nil || string(data) != "two" || f.Version != 2 || f.SHA256 != "sha256:"+hex.EncodeToString(two[:]) || f.Size != 3 ||
		f.ContentType != "text/plain" || !f.CreatedAt.Equal(time.Date(2026, 10, 2, 8, 0, 0, 0, time.UTC)) {
		t.Errorf("GetFile = %+v, %q, %v", f.FileInfo, data, err)
	}

	if err := c.DeleteFile(ctx, "notes.txt"); err != nil {
		t.Fatalf("DeleteFile: %v", err)
	}
	if _, err := c.GetFile(ctx, "notes.txt", GetFileOptions{}); !errors.Is(err, ErrNotFound) {
		t.Errorf("GetFile of a deleted file: %v, want ErrNotFound", err)
	}
	if err := c.DeleteFile(ctx, "notes.txt"); !errors.Is(err, ErrNotFound) {
		t.Errorf("DeleteFile twice: %v, want ErrNotFound", err)
	}
	var cerr *Error
	if _, err := c.GetFile(ctx, "missing", GetFileOptions{}); !errors.As(err, &cerr) || cerr.StatusCode != 404 || cerr.Code != "not-found" ||
		cerr.Message != "not-found /v1/files/missing" {
		t.Errorf("GetFile of a missing file: %#v", err)
	}
}

// Content that does not match the hash its answer names is an error at its end.
func TestGetFileChecksTheContent(t *testing.T) {
	_, b := newFileStore(t)
	c := newTestClient(t, time.Second, b.srv.URL)
	ctx := context.Background()
	if _, _, err := c.PutFile(ctx, "corrupt", strings.NewReader("content"), PutOptions{}); err != nil {
		t.Fatal(err)
	}
	f, err := c.GetFile(ctx, "corrupt", GetFileOptions{})
	if err != nil {
		t.Fatalf("GetFile: %v", err)
	}
	defer f.Body.Close()
	if _, err := io.ReadAll(f.Body); err == nil || !strings.Contains(err.Error(), "does not match its hash") {
		t.Errorf("reading corrupt content: %v", err)
	}
}

func TestGetFilePicksAVersion(t *testing.T) {
	rec := newRecorder(t)
	c := newTestClient(t, time.Second, rec.srv.URL)
	ctx := context.Background()
	for _, tc := range []struct {
		o    GetFileOptions
		want string
	}{
		{GetFileOptions{}, ""},
		{GetFileOptions{Version: 7}, "version=7"},
		{GetFileOptions{At: time.Date(2026, 9, 1, 12, 0, 0, 500, time.UTC)}, "at=2026-09-01T12%3A00%3A00.0000005Z"},
	} {
		f, err := c.GetFile(ctx, "a/b", tc.o)
		if err != nil {
			t.Fatalf("GetFile: %v", err)
		}
		f.Body.Close()
		if got := rec.last(t); got.URL.Path != "/v1/files/a/b" || got.URL.RawQuery != tc.want {
			t.Errorf("GetFile(%+v) asked %s?%s, want ?%s", tc.o, got.URL.Path, got.URL.RawQuery, tc.want)
		}
	}
}

func TestInvalidFileNamesAreRefused(t *testing.T) {
	c := newTestClient(t, time.Second, refusing(t))
	for _, name := range []string{"", "/a", "a/", "a//b", "a/./b", "../etc/passwd", "a/..", "a\x00b", "tab\there", "\xff",
		strings.Repeat("x", 1025)} {
		if _, err := c.GetFile(context.Background(), name, GetFileOptions{}); err == nil || !strings.Contains(err.Error(), "invalid file name") {
			t.Errorf("GetFile(%q): %v", name, err)
		}
	}
	if _, err := filePath(strings.Repeat("x", 1024)); err != nil {
		t.Errorf("1024 bytes: %v", err)
	}
}

func TestStatusAndStepDown(t *testing.T) {
	leader := newInstance(t, func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/status":
			fmt.Fprint(w, `{"instance":"a","role":"leader","epoch":3,"seq":120,"leader":{"instance":"a","url":"http://cortex_a:8100",
				"since":"2026-10-02T08:00:00.000000Z"},"follower":null,"hosts":[{"host":"qis.b-tu.de","queue":0}],"version":"1.0",
				"started_at":"2026-10-02T07:59:58.000000Z"}`)
		case "/v1/admin/step-down":
			if r.Method != http.MethodPost {
				t.Errorf("step-down with %s", r.Method)
			}
			w.WriteHeader(http.StatusOK)
		}
	})
	follower := newInstance(t, func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/status":
			// A field of another shape than the client's does not lose the rest.
			fmt.Fprint(w, `{"instance":"b","role":"follower","epoch":3,"seq":118,"follower":{"lag_seconds":1.5,"lag_entries":2,"state":"following"},
				"version":{"cortex":"1.0"}}`)
		default:
			w.Header().Set("Cortex-Error", "not-leader")
			w.WriteHeader(http.StatusConflict)
			fmt.Fprint(w, `{"error":"not-leader","message":"b follows a"}`)
		}
	})
	ctx := context.Background()

	st, err := newTestClient(t, time.Second, leader.srv.URL).Status(ctx)
	if err != nil {
		t.Fatalf("Status: %v", err)
	}
	if st.Instance != "a" || st.Role != "leader" || st.Epoch != 3 || st.Seq != 120 || st.Leader == nil || st.Leader.URL != "http://cortex_a:8100" ||
		st.Follower != nil || st.Version != "1.0" || st.StartedAt.IsZero() || !strings.Contains(string(st.Raw), "qis.b-tu.de") {
		t.Errorf("Status = %+v", st)
	}
	st, err = newTestClient(t, time.Second, follower.srv.URL).Status(ctx)
	if err != nil || st.Role != "follower" || st.Follower == nil || st.Follower.LagSeconds != 1.5 || st.Follower.LagEntries != 2 || st.Version != "" {
		t.Errorf("Status of the follower = %+v, %v", st, err)
	}

	if err := newTestClient(t, time.Second, leader.srv.URL).StepDown(ctx); err != nil {
		t.Errorf("StepDown at the leader: %v", err)
	}
	err = newTestClient(t, time.Second, follower.srv.URL).StepDown(ctx)
	if !errors.Is(err, ErrNotLeader) || err.Error() != "cortex: 409 not-leader: b follows a" {
		t.Errorf("StepDown at the follower: %v, want ErrNotLeader", err)
	}
}
