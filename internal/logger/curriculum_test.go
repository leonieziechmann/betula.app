package logger

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCurriculumAuditPreservesWarningsAndRedactsSecrets(t *testing.T) {
	a, err := NewCurriculumAudit(t.TempDir(), "test-secret")
	if err != nil {
		t.Fatal(err)
	}
	defer a.Close()
	for _, e := range []CurriculumEvent{{Level: "warning", Code: "season", ProgramID: "id", Program: "SÄ", Message: "API test-secret", Action: "check test-secret"}, {Level: "info", Code: "saved", ProgramID: "id", Status: "saved_with_warnings"}} {
		if err = a.Record(e); err != nil {
			t.Fatal(err)
		}
	}
	if err = a.Finish("completed"); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"events.jsonl", "summary.json", "review.md"} {
		b, err := os.ReadFile(filepath.Join(a.Dir, name))
		if err != nil {
			t.Fatal(err)
		}
		if strings.Contains(string(b), "test-secret") {
			t.Fatal("secret leaked")
		}
	}
	b, _ := os.ReadFile(filepath.Join(a.Dir, "summary.json"))
	var s struct {
		Counts map[string]int `json:"counts"`
		Issues map[string]int `json:"issue_counts"`
	}
	if err = json.Unmarshal(b, &s); err != nil {
		t.Fatal(err)
	}
	if s.Counts["saved_with_warnings"] != 1 || s.Issues["season"] != 1 {
		t.Fatalf("bad summary: %s", b)
	}
}

func TestFailureCategories(t *testing.T) {
	for input, want := range map[string]string{"status 429 quota": "api_quota", "no supported ruled semester/ECTS table found": "unsupported_layout", "incomplete table p2": "incomplete_table", "context deadline exceeded": "api_transport"} {
		code, action := ClassifyCurriculumFailure(input)
		if code != want || action == "" {
			t.Errorf("%s: %s", input, code)
		}
	}
}

func TestAuditCanRebuildSummaryAfterInterruption(t *testing.T) {
	a, err := NewCurriculumAudit(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err = a.Record(CurriculumEvent{Level: "info", Code: "scan_started", ProgramID: "p", Status: "running"}); err != nil {
		t.Fatal(err)
	}
	dir := a.Dir
	a.Close()
	a, err = OpenCurriculumAudit(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer a.Close()
	if len(a.Events) != 1 || a.Outcomes["p"].Status != "running" || a.Started.IsZero() {
		t.Fatal("lost audit context")
	}
	if err = a.Finish("interrupted"); err != nil {
		t.Fatal(err)
	}
	b, err := os.ReadFile(filepath.Join(dir, "summary.json"))
	if err != nil || !strings.Contains(string(b), "interrupted") {
		t.Fatalf("missing interruption summary: %s %v", b, err)
	}
}
