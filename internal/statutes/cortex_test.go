package statutes

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	cortexclient "github.com/leonieziechmann/betula/internal/cortex/client"
	"github.com/leonieziechmann/betula/internal/cortex/client/cortextest"
)

// Through Cortex the download names its source and passes on its headers; a challenge page,
// which Cortex refuses to take from OPUS for a PDF (wrong-type), is bot protection as
// without Cortex, and another failure of Cortex an error that names it.
func TestDownloadThroughCortex(t *testing.T) {
	opus := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/pdf")
		_, _ = w.Write([]byte("%PDF-1.7 content"))
	}))
	defer opus.Close()
	cortex := cortextest.NewServer()
	defer cortex.Close()
	c, err := cortexclient.New(cortex.URL, cortexclient.Options{})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	client := c.HTTPClient(cortexclient.FetchOptions{Mode: cortexclient.ModeRefresh, MaxAge: time.Hour, Stale: cortexclient.StaleNever}, 3*time.Minute)
	dir := t.TempDir()
	ctx := context.Background()

	docURL := opus.URL + "/opus4-btu/files/6707/12_Informatik_B.Sc.pdf"
	path, cached, err := Download(ctx, client, dir, "Informatik", docURL, true)
	if err != nil || cached {
		t.Fatalf("Download = %q, %v, %v", path, cached, err)
	}
	if body, _ := os.ReadFile(path); string(body) != "%PDF-1.7 content" {
		t.Errorf("saved body = %q", body)
	}
	fetches := cortex.Fetches()
	if len(fetches) != 1 {
		t.Fatalf("Cortex got %d fetches", len(fetches))
	}
	f := fetches[0]
	if f.URL != docURL || f.Source != "statute" || f.Mode != "refresh" || f.Stale != "never" ||
		f.Header.Get("User-Agent") != UserAgent || f.Header.Get("Accept") != "application/pdf,application/octet-stream,*/*" {
		t.Errorf("Cortex got %+v with headers %v", f.Query, f.Header)
	}

	cortex.Fail(http.StatusBadGateway, "wrong-type")
	if _, _, err := Download(ctx, client, dir, "Informatik", opus.URL+"/files/2/challenge.pdf", false); !errors.Is(err, ErrBotProtection) {
		t.Errorf("wrong-type: err = %v, want ErrBotProtection", err)
	}
	cortex.Fail(http.StatusBadGateway, "upstream-failed")
	_, _, err = Download(ctx, client, dir, "Informatik", opus.URL+"/files/3/po.pdf", false)
	if err == nil || errors.Is(err, ErrBotProtection) || !strings.Contains(err.Error(), "unexpected status 502 (Cortex: upstream-failed)") {
		t.Errorf("upstream-failed: err = %v", err)
	}
	for _, name := range []string{"2_challenge.pdf", "3_po.pdf"} {
		if Exists(LocalPath(dir, "Informatik", opus.URL+"/files/"+name[:1]+"/"+name[2:])) {
			t.Errorf("a failed download left %s behind", name)
		}
	}
}
