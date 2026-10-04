package curriculumscan

import (
	"path/filepath"
	"testing"
)

func TestNormalizePathMatchesHost(t *testing.T) {
	windows := `statutes\Informatik\6707_12_Informatik_B.Sc.pdf`
	unix := "statutes/Informatik/6707_12_Informatik_B.Sc.pdf"
	want := unix
	if filepath.Separator != '/' {
		want = windows
	}
	for _, in := range []string{windows, unix} {
		if got := NormalizePath(in); got != want {
			t.Errorf("NormalizePath(%q) = %q, want %q", in, got, want)
		}
	}
}
