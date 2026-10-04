package client

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"
)

func TestImportSendsTheAnswerAndItsTimes(t *testing.T) {
	var got *http.Request
	var body string
	in := newInstance(t, func(w http.ResponseWriter, r *http.Request) {
		data, _ := io.ReadAll(r.Body)
		got, body = r, string(data)
		w.WriteHeader(http.StatusCreated)
		fmt.Fprint(w, `{"result":"created","entry":{"id":3},"version":{"id":7,"sha256":"sha256:abc"}}`)
	})
	c := newTestClient(t, 0, in.srv.URL)
	first := time.Date(2026, 9, 29, 23, 21, 45, 0, time.FixedZone("CEST", 2*60*60))
	last := time.Date(2026, 10, 4, 3, 41, 1, 500000000, time.UTC)
	res, err := c.Import(context.Background(), "https://www.b-tu.de/modul/11101", strings.NewReader("<html>Übung</html>"), ImportOptions{
		Status: 200, FetchedAt: first, CheckedAt: last, Source: "module_page", ContentType: "text/html"})
	if err != nil || res != (ImportResult{Result: ImportCreated, Version: 7, SHA256: "sha256:abc"}) {
		t.Fatalf("Import = %+v, %v", res, err)
	}
	q := got.URL.Query()
	if got.Method != http.MethodPut || got.URL.Path != "/v1/entries" || q.Get("url") != "https://www.b-tu.de/modul/11101" ||
		q.Get("status") != "200" || q.Get("fetched_at") != "2026-09-29T21:21:45Z" || q.Get("checked_at") != "2026-10-04T03:41:01.5Z" ||
		q.Get("source") != "module_page" || q.Has("accept") || q.Has("expect") || got.Header.Get("Content-Type") != "text/html" ||
		body != "<html>Übung</html>" {
		t.Fatalf("the request: %s %s %v, body %q", got.Method, got.URL, got.Header, body)
	}

	in.set(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cortex-Error", "bad-request")
		w.WriteHeader(http.StatusBadRequest)
		fmt.Fprint(w, `{"error":"bad-request","message":"checked_at lies in the future"}`)
	})
	_, err = c.Import(context.Background(), "https://www.b-tu.de/modul/1", strings.NewReader("x"), ImportOptions{Status: 200, FetchedAt: first, CheckedAt: last})
	var ce *Error
	if !errors.As(err, &ce) || ce.StatusCode != http.StatusBadRequest || ce.Code != "bad-request" {
		t.Fatalf("a refused import: %v", err)
	}
	if _, err := c.Import(context.Background(), "https://www.b-tu.de/modul/1", strings.NewReader("x"), ImportOptions{Status: 200}); err == nil {
		t.Fatal("an import without times was sent")
	}
}
