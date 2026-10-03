package statutes

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestLocalPathKeepsTheOpusNumber(t *testing.T) {
	got := LocalPath("statutes", "Künstliche Intelligenz Technologie", "https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf")
	want := filepath.Join("statutes", "Künstliche_Intelligenz_Technologie", "6707_12_Informatik_B.Sc.pdf")
	if got != want {
		t.Errorf("LocalPath = %q, want %q", got, want)
	}
	if got := LocalPath("s", "A/B: C", "https://opus4.kobv.de/opus4-btu/frontdoor/deliver/index/docId/55/file/plan?x=1"); got != filepath.Join("s", "A_B__C", "55_plan.pdf") {
		t.Errorf("LocalPath with odd characters = %q", got)
	}
}

func TestDownload(t *testing.T) {
	var hits int
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits++
		if r.Header.Get("User-Agent") != UserAgent {
			t.Errorf("User-Agent = %q", r.Header.Get("User-Agent"))
		}
		switch r.URL.Path {
		case "/files/1/po.pdf":
			w.Header().Set("Content-Type", "application/pdf")
			_, _ = w.Write([]byte("%PDF-1.7 content"))
		case "/files/2/challenge.pdf":
			w.Header().Set("Content-Type", "text/html")
			_, _ = w.Write([]byte("<html>Checking that you are not a bot</html>"))
		default:
			_, _ = w.Write([]byte("hello"))
		}
	}))
	defer srv.Close()
	dir := t.TempDir()
	ctx := context.Background()

	path, cached, err := Download(ctx, nil, dir, "Informatik", srv.URL+"/files/1/po.pdf", false)
	if err != nil || cached || !Exists(path) {
		t.Fatalf("Download = %q, %v, %v", path, cached, err)
	}
	if body, _ := os.ReadFile(path); string(body) != "%PDF-1.7 content" {
		t.Errorf("saved body = %q", body)
	}
	if _, cached, err := Download(ctx, nil, dir, "Informatik", srv.URL+"/files/1/po.pdf", false); err != nil || !cached || hits != 1 {
		t.Errorf("second Download: cached=%v err=%v hits=%d", cached, err, hits)
	}

	// The dual variant shares the document: the existing copy is found, not downloaded again.
	if shared, cached, err := Download(ctx, nil, dir, "Informatik - dual", srv.URL+"/files/1/po.pdf", false); err != nil || !cached || shared != path || hits != 1 {
		t.Errorf("shared document: %q cached=%v err=%v hits=%d", shared, cached, err, hits)
	}

	if _, _, err := Download(ctx, nil, dir, "Informatik", srv.URL+"/files/2/challenge.pdf", false); !errors.Is(err, ErrBotProtection) {
		t.Errorf("challenge page: err = %v, want ErrBotProtection", err)
	}
	if _, _, err := Download(ctx, nil, dir, "Informatik", srv.URL+"/files/3/not-a-pdf.pdf", false); err == nil {
		t.Error("a non-PDF response was saved")
	}
	if Exists(LocalPath(dir, "Informatik", srv.URL+"/files/3/not-a-pdf.pdf")) {
		t.Error("a rejected download left a file behind")
	}
}
