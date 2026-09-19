package web

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSPADirectLinksAndAssets(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "index.html"), []byte("<html>Rust app</html>"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "app.js"), []byte("// asset"), 0600); err != nil {
		t.Fatal(err)
	}
	handler := spaHandler(dir)
	for _, path := range []string{"/", "/?program=old", "/catalogue?duration=2&grading=benotet", "/catalouge?data=%7B%7D", "/course/11101", "/course/A%2BB%2FC", "/study-programm/S%C3%84_2024/plan", "/study-programm/bsc-informatik-2008/plan", "/study-programm/id/electives", "/study-programm/id/modules", "/study-programm/id"} {
		t.Run(path, func(t *testing.T) {
			w := httptest.NewRecorder()
			handler.ServeHTTP(w, httptest.NewRequest(http.MethodGet, path, nil))
			if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "Rust app") {
				t.Fatalf("deep link: %d %s", w.Code, w.Body.String())
			}
		})
	}
	for _, path := range []string{"/static/missing.js", "/api/unknown", "/unknown", "/course/", "/study-programm/id/unknown", "/course/id/extra"} {
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, httptest.NewRequest(http.MethodGet, path, nil))
		if w.Code != http.StatusNotFound {
			t.Errorf("%s: got %d, want 404", path, w.Code)
		}
	}
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, httptest.NewRequest(http.MethodGet, "/app.js", nil))
	if w.Code != http.StatusOK || w.Body.String() != "// asset" {
		t.Fatalf("asset: %d %s", w.Code, w.Body.String())
	}
	w = httptest.NewRecorder()
	handler.ServeHTTP(w, httptest.NewRequest(http.MethodPost, "/course/11101", nil))
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("POST: %d", w.Code)
	}
}
