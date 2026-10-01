package embed

import (
	"bufio"
	"context"
	"encoding/hex"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

// The module is the crate's: it compiles and has the encoder's exports.
func TestTheModuleHasTheEncoder(t *testing.T) {
	model := filepath.Join(t.TempDir(), "model.bin")
	if err := os.WriteFile(model, []byte("E5Q1 is not this"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err := Load(context.Background(), model, 1)
	if err == nil || !strings.Contains(err.Error(), "not a packed model") {
		t.Errorf("Load(not a model) = %v, want „not a packed model“ (an older semantic.wasm lacks the exports)", err)
	}
	if _, err := Load(context.Background(), filepath.Join(t.TempDir(), "missing.bin"), 1); err == nil {
		t.Error("Load(missing file) did not fail")
	}
}

// The vectors are the bits the crate computes natively (`embed MODEL --passages`, the golden
// file), whichever instance computes them and however many at once. The model is not in the
// repository: RADIX_TEST_EMBED_MODEL names the server's (semantic/README.md).
func TestTheVectorsAreTheCratesBits(t *testing.T) {
	path := os.Getenv("RADIX_TEST_EMBED_MODEL")
	if path == "" {
		t.Skip("RADIX_TEST_EMBED_MODEL is not set")
	}
	ctx := context.Background()
	e, err := Load(ctx, path, 3)
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close(ctx)
	if e.Dims() != 384 || len(e.ID()) != 16 {
		t.Errorf("dims %d, id %q", e.Dims(), e.ID())
	}

	passages := lines(t, "testdata/passages.txt")
	golden := lines(t, "testdata/passages.golden")
	if len(passages) != len(golden) {
		t.Fatalf("%d passages, %d golden vectors", len(passages), len(golden))
	}
	got := make([]string, len(passages))
	var wg sync.WaitGroup
	for i, p := range passages {
		wg.Go(func() {
			scale, packed, err := e.EmbedPassage(ctx, p)
			if err != nil {
				t.Errorf("passage %d: %v", i+1, err)
				return
			}
			// Two values of -7..7 a byte, as nibbles v + 8: of unit length, up to 4 bits' rounding.
			var norm float64
			for _, b := range packed {
				for _, v := range []int{int(b&0x0f) - 8, int(b>>4) - 8} {
					norm += float64(v*v) * float64(scale) * float64(scale)
				}
			}
			if len(packed) != e.Dims()/2 || math.Abs(math.Sqrt(norm)-1) > 0.1 {
				t.Errorf("passage %d: %d bytes, length %.4f, want %d and about 1", i+1, len(packed), math.Sqrt(norm), e.Dims()/2)
			}
			got[i] = fmt.Sprintf("%08x\t%s", math.Float32bits(scale), hex.EncodeToString(packed))
		})
	}
	wg.Wait()
	for i := range passages {
		if got[i] != golden[i] {
			t.Errorf("passage %d (%.40q…) differs from the crate's bits", i+1, passages[i])
		}
	}
}

func lines(t *testing.T, path string) []string {
	t.Helper()
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var out []string
	s := bufio.NewScanner(f)
	s.Buffer(make([]byte, 1<<20), 1<<20)
	for s.Scan() {
		out = append(out, s.Text())
	}
	if err := s.Err(); err != nil {
		t.Fatal(err)
	}
	return out
}
