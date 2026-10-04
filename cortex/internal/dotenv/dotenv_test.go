package dotenv

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestLoadDotEnv(t *testing.T) {
	for _, name := range []string{"CORTEX_ADDR", "CORTEX_QUOTED", "CORTEX_ALREADY_SET"} {
		name := name
		if old, ok := os.LookupEnv(name); ok {
			t.Cleanup(func() { os.Setenv(name, old) })
		} else {
			t.Cleanup(func() { os.Unsetenv(name) })
		}
		os.Unsetenv(name)
	}
	os.Setenv("CORTEX_ALREADY_SET", "from the real environment")

	path := filepath.Join(t.TempDir(), ".env")
	content := "# development only\n\nexport CORTEX_ADDR=127.0.0.1:9100   # another port\nCORTEX_QUOTED=\"two words # not a comment\"\nCORTEX_ALREADY_SET=from the file\n"
	if err := os.WriteFile(path, []byte(content), 0600); err != nil {
		t.Fatal(err)
	}
	set, err := LoadDotEnv(path)
	if err != nil {
		t.Fatalf("LoadDotEnv failed: %v", err)
	}
	if want := []string{"CORTEX_ADDR", "CORTEX_QUOTED"}; !reflect.DeepEqual(set, want) {
		t.Errorf("set = %v, want %v", set, want)
	}
	if got := os.Getenv("CORTEX_ADDR"); got != "127.0.0.1:9100" {
		t.Errorf("CORTEX_ADDR = %q", got)
	}
	if got := os.Getenv("CORTEX_QUOTED"); got != "two words # not a comment" {
		t.Errorf("CORTEX_QUOTED = %q", got)
	}
	// A deployment that configures its environment properly is never overridden by a stray file.
	if got := os.Getenv("CORTEX_ALREADY_SET"); got != "from the real environment" {
		t.Errorf("CORTEX_ALREADY_SET = %q", got)
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
