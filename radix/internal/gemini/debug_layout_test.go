package gemini

import (
	"context"
	"encoding/json"
	"github.com/ledongthuc/pdf"
	"os"
	"testing"
)

func TestDebugLayout(t *testing.T) {
	path := os.Getenv("RADIX_DEBUG_PDF")
	if path == "" {
		t.Skip()
	}
	f, r, e := pdf.Open(path)
	if e != nil {
		t.Fatal(e)
	}
	defer f.Close()
	out, e := os.Create("../../.cache/debug-tables.jsonl")
	if e != nil {
		t.Fatal(e)
	}
	defer out.Close()
	enc := json.NewEncoder(out)
	for p := 1; p <= r.NumPage(); p++ {
		g, e := readPageGeometry(context.Background(), r.Page(p))
		if e != nil {
			t.Fatal(e)
		}
		tabs, e := tableGeometry(context.Background(), g.edges, g.glyphs)
		if e != nil {
			t.Fatal(e)
		}
		for i, tab := range tabs {
			enc.Encode(map[string]any{"page": p, "table": i + 1, "rows": tab.rows})
		}
		enc.Encode(map[string]any{"page": p, "text": textInBox(g.glyphs, nil)})
	}
}
