package version

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"sync"
)

// Build names the binary that runs: a hash of its own executable. Radix is the same
// version across many releases, but every release that changes a parser or a rule is a
// different binary. A build stores it (meta key radix_build), so that a new release can
// tell at start that the canonical tables were derived by another one and build them
// again from the archive, instead of waiting for its first crawl (service.Rebuild).
// "" when the executable cannot be read; then every start counts as a new release.
func Build() string {
	buildOnce.Do(func() { buildHash = executableHash() })
	return buildHash
}

var (
	buildOnce sync.Once
	buildHash string
)

func executableHash() string {
	path, err := os.Executable()
	if err != nil {
		return ""
	}
	f, err := os.Open(path)
	if err != nil {
		return ""
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return ""
	}
	return hex.EncodeToString(h.Sum(nil))[:16]
}
