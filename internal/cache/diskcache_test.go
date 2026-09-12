package cache

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestDiskCache(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "btu_cache_test_*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	c, err := NewDiskCache(tempDir)
	if err != nil {
		t.Fatalf("NewDiskCache failed: %v", err)
	}

	key := "test:item:1"
	val := []byte("hello world")

	// 1. Get non-existent
	data, ok, err := c.Get(key)
	if err != nil {
		t.Fatalf("unexpected error on non-existent key: %v", err)
	}
	if ok || data != nil {
		t.Fatalf("expected false, got ok=%v data=%v", ok, data)
	}

	// 2. Set with TTL
	if err := c.Set(key, val, 1*time.Hour); err != nil {
		t.Fatalf("failed to set cache item: %v", err)
	}

	// 3. Get existing
	data, ok, err = c.Get(key)
	if err != nil || !ok {
		t.Fatalf("expected ok=true, got ok=%v, err=%v", ok, err)
	}
	if string(data) != string(val) {
		t.Fatalf("expected %q, got %q", string(val), string(data))
	}

	// 4. Test expired entry
	shortKey := "test:short"
	if err := c.Set(shortKey, []byte("short lived"), 10*time.Millisecond); err != nil {
		t.Fatalf("failed to set short TTL: %v", err)
	}
	time.Sleep(30 * time.Millisecond)
	data, ok, err = c.Get(shortKey)
	if err != nil {
		t.Fatalf("unexpected error on expired key: %v", err)
	}
	if ok || data != nil {
		t.Fatalf("expected expired item to return ok=false, got ok=%v", ok)
	}

	// 5. Delete
	if err := c.Delete(key); err != nil {
		t.Fatalf("failed to delete key: %v", err)
	}
	_, ok, _ = c.Get(key)
	if ok {
		t.Fatalf("expected key to be deleted")
	}

	// 6. Clear
	_ = c.Set("k1", []byte("v1"), 0)
	_ = c.Set("k2", []byte("v2"), 0)
	if err := c.Clear(); err != nil {
		t.Fatalf("failed to clear cache: %v", err)
	}
	entries, _ := os.ReadDir(filepath.Join(tempDir))
	for _, e := range entries {
		if filepath.Ext(e.Name()) == ".cache" {
			t.Fatalf("expected no .cache files after Clear(), found %s", e.Name())
		}
	}
}
