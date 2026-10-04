package server

import (
	"encoding/json"
	"net/http"
	"net/url"
	"strings"
	"testing"
	"time"
)

// importURL is the PUT /v1/entries URL of target with the parameters given as name, value pairs.
func importURL(c *testCortex, target string, params ...string) string {
	q := url.Values{"url": {target}}
	for i := 0; i+1 < len(params); i += 2 {
		q.Set(params[i], params[i+1])
	}
	return c.URL + "/v1/entries?" + q.Encode()
}

// importAnswer gives Cortex an answer another program fetched first at first and last at last,
// and returns the answer and what it says.
func importAnswer(t *testing.T, c *testCortex, target string, status, content string, first, last time.Time, extra ...string) (*http.Response, importJSON) {
	t.Helper()
	params := append([]string{"status", status, "fetched_at", first.Format(time.RFC3339Nano), "checked_at", last.Format(time.RFC3339Nano)}, extra...)
	resp, body := do(t, http.MethodPut, importURL(c, target, params...), strings.NewReader(content), "Content-Type", "text/html; charset=utf-8")
	var out importJSON
	if resp.StatusCode == http.StatusOK || resp.StatusCode == http.StatusCreated {
		if err := json.Unmarshal([]byte(body), &out); err != nil {
			t.Fatalf("PUT /v1/entries for %s: %s", target, body)
		}
	}
	return resp, out
}

type importJSON struct {
	Result  string      `json:"result"`
	Entry   entryJSON   `json:"entry"`
	Version versionJSON `json:"version"`
}

func TestAnImportedAnswerIsServedOfflineWithItsTimes(t *testing.T) {
	c := newTestCortex(t)
	const target = "https://www.b-tu.de/modul/11101"
	first, last := t0.Add(-72*time.Hour), t0.Add(-24*time.Hour)
	created := `cortex_imports_total{source="module_page",result="created"}`
	unchanged := `cortex_imports_total{source="module_page",result="unchanged"}`
	before := scrape(t, c)

	resp, out := importAnswer(t, c, target, "200", "<html>Übung</html>", first, last, "source", "module_page")
	if resp.StatusCode != http.StatusCreated || out.Result != "created" || out.Entry.Source != "module_page" ||
		out.Version.FetchedAt != "2026-09-29T10:00:00.000000Z" || out.Version.CheckedAt != "2026-10-01T10:00:00.000000Z" ||
		out.Version.SHA256 != "sha256:"+sha("<html>Übung</html>") || resp.Header.Get("Cortex-Version") != itoa(out.Version.ID) {
		t.Fatalf("import: %d %+v", resp.StatusCode, out)
	}

	resp, body := get(t, c.fetchURL(target, "mode", "offline", "source", "module_page"))
	if resp.StatusCode != http.StatusOK || body != "<html>Übung</html>" || resp.Header.Get("Cache-Status") != "Cortex; hit" ||
		resp.Header.Get("Cortex-Fetched-At") != "2026-09-29T10:00:00.000000Z" || resp.Header.Get("Cortex-Checked-At") != "2026-10-01T10:00:00.000000Z" ||
		resp.Header.Get("Content-Type") != "text/html; charset=utf-8" || resp.Header.Get("Age") != "86400" {
		t.Fatalf("offline fetch of the import: %d %q %v", resp.StatusCode, body, resp.Header)
	}

	if resp, again := importAnswer(t, c, target, "200", "<html>Übung</html>", first, last, "source", "module_page"); resp.StatusCode != http.StatusOK ||
		again.Result != "unchanged" || again.Version.ID != out.Version.ID {
		t.Fatalf("the same import again: %d %+v", resp.StatusCode, again)
	}
	after := scrape(t, c)
	if after[created]-before[created] != 1 || after[unchanged]-before[unchanged] != 1 {
		t.Errorf("imports counted: created %v, unchanged %v", after[created]-before[created], after[unchanged]-before[unchanged])
	}

	// A page that was gone is an answer too: offline it is the host's 404, not Cortex's error.
	if resp, out := importAnswer(t, c, "https://www.b-tu.de/modul/404", "404", "", first, last); resp.StatusCode != http.StatusCreated || out.Version.Status != 404 {
		t.Fatalf("import of a 404: %d %+v", resp.StatusCode, out)
	}
	resp, body = get(t, c.fetchURL("https://www.b-tu.de/modul/404", "mode", "offline"))
	if resp.StatusCode != http.StatusNotFound || resp.Header.Get("Cortex-Error") != "" || body != "" {
		t.Fatalf("offline fetch of an imported 404: %d %q %v", resp.StatusCode, body, resp.Header)
	}

	// What another program had is not what a newer answer of Cortex's own is.
	c.clock.advance(time.Hour)
	if resp, older := importAnswer(t, c, target, "200", "<html>alt</html>", first.Add(-time.Hour), first); resp.StatusCode != http.StatusOK || older.Result != "older" ||
		older.Version.ID != out.Version.ID {
		t.Fatalf("an older import of other content: %d %+v", resp.StatusCode, older)
	}
}

func TestAnImportThatDoesNotFitIsRefused(t *testing.T) {
	c := newTestCortex(t)
	const target = "https://www.b-tu.de/modul/11101"
	first, last := t0.Add(-time.Hour).Format(time.RFC3339), t0.Format(time.RFC3339)
	for _, tc := range []struct {
		name   string
		params []string
	}{
		{"no status", []string{"fetched_at", first, "checked_at", last}},
		{"a status Cortex does not keep", []string{"status", "500", "fetched_at", first, "checked_at", last}},
		{"no times", []string{"status", "200"}},
		{"fetched after checked", []string{"status", "200", "fetched_at", last, "checked_at", first}},
		{"checked in the future", []string{"status", "200", "fetched_at", first, "checked_at", t0.Add(time.Hour).Format(time.RFC3339)}},
		{"a bad source", []string{"status", "200", "fetched_at", first, "checked_at", last, "source", "Module Page"}},
	} {
		resp, body := do(t, http.MethodPut, importURL(c, target, tc.params...), strings.NewReader("x"))
		if resp.StatusCode != http.StatusBadRequest || resp.Header.Get("Cortex-Error") != codeBadRequest {
			t.Errorf("%s: %d %s, want 400 bad-request", tc.name, resp.StatusCode, body)
		}
	}
	resp, body := do(t, http.MethodPut, c.URL+"/v1/entries?status=200&fetched_at="+first+"&checked_at="+last, strings.NewReader("x"))
	wantStatus(t, resp, body, http.StatusBadRequest, codeBadRequest)
	resp, body = do(t, http.MethodPut, importURL(c, target, "status", "200", "fetched_at", first, "checked_at", last, "expect", "sha256:"+sha("y")), strings.NewReader("x"))
	wantStatus(t, resp, body, http.StatusUnprocessableEntity, codeHashMismatch)
	if versionsOf(t, c, target) != 0 {
		t.Fatal("a refused import was stored")
	}
}

func TestAFollowerForwardsAnImportToTheLeader(t *testing.T) {
	leader, follower, _ := newPair(t)
	const target = "https://qis.b-tu.de/qisserver/rds?state=wtree&search=1"
	resp, out := importAnswer(t, follower, target, "200", "the tree", t0.Add(-time.Hour), t0, "source", "qis_tree")
	if resp.StatusCode != http.StatusCreated || out.Result != "created" || resp.Header.Get("Cortex-Instance") != "a; role=leader" {
		t.Fatalf("an import sent to the follower: %d %+v %v", resp.StatusCode, out, resp.Header)
	}
	if versionsOf(t, leader, target) != 1 || versionsOf(t, follower, target) != 0 {
		t.Fatal("the leader did not store the import, or the follower wrote it itself")
	}
}
