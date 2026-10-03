package version

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"sync"
)

// Build names the binary that runs: a hash of its own executable, shown in /status and in
// cortex_build_info. "" when the executable cannot be read.
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
