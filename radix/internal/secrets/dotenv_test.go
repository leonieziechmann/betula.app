package secrets

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/zalando/go-keyring"
)

func TestLoadDotEnv(t *testing.T) {
	keyring.MockInit()
	dockerSecretsDir = filepath.Join(t.TempDir(), "no-docker")
	t.Cleanup(func() { dockerSecretsDir = "/run/secrets" })
	for _, name := range []string{"GEMINI_API_KEY", "RADIX_OFFPEAK", "RADIX_QUOTED", "RADIX_ALREADY_SET"} {
		name := name
		if old, ok := os.LookupEnv(name); ok {
			t.Cleanup(func() { os.Setenv(name, old) })
		} else {
			t.Cleanup(func() { os.Unsetenv(name) })
		}
		os.Unsetenv(name)
	}
	os.Setenv("RADIX_ALREADY_SET", "from the real environment")

	path := filepath.Join(t.TempDir(), ".env")
	content := "# development only\n\nGEMINI_API_KEY=dev-key-123\nexport RADIX_OFFPEAK=any   # crawl at any time\nRADIX_QUOTED=\"two words # not a comment\"\nRADIX_ALREADY_SET=from the file\n"
	if err := os.WriteFile(path, []byte(content), 0600); err != nil {
		t.Fatal(err)
	}

	set, err := LoadDotEnv(path)
	if err != nil {
		t.Fatalf("LoadDotEnv failed: %v", err)
	}
	if want := []string{"GEMINI_API_KEY", "RADIX_OFFPEAK", "RADIX_QUOTED"}; !reflect.DeepEqual(set, want) {
		t.Errorf("set = %v, want %v", set, want)
	}
	if got := os.Getenv("RADIX_OFFPEAK"); got != "any" {
		t.Errorf("RADIX_OFFPEAK = %q", got)
	}
	if got := os.Getenv("RADIX_QUOTED"); got != "two words # not a comment" {
		t.Errorf("RADIX_QUOTED = %q", got)
	}
	// A deployment that configures its environment properly is never overridden by a stray file.
	if got := os.Getenv("RADIX_ALREADY_SET"); got != "from the real environment" {
		t.Errorf("RADIX_ALREADY_SET = %q", got)
	}

	value, source, err := Resolve(GeminiAPIKey)
	if err != nil || value != "dev-key-123" || !strings.Contains(string(source), "development file") {
		t.Errorf("Resolve = %q, %q, %v", value, source, err)
	}

	if set, err := LoadDotEnv(filepath.Join(t.TempDir(), "missing.env")); err != nil || set != nil {
		t.Errorf("a missing file must be fine: %v, %v", set, err)
	}
	bad := filepath.Join(t.TempDir(), "bad.env")
	_ = os.WriteFile(bad, []byte("this is not an assignment\n"), 0600)
	if _, err := LoadDotEnv(bad); err == nil {
		t.Error("a malformed line was accepted")
	}
}
