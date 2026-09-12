package cache

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// DiskCache implements Cache using files on the local filesystem.
// Each file stores an 8-byte big-endian int64 Unix millisecond expiry timestamp
// followed by the raw cached payload.
type DiskCache struct {
	dir string
	mu  sync.RWMutex
}

// NewDiskCache creates or opens a disk cache at the specified directory.
func NewDiskCache(dir string) (*DiskCache, error) {
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create cache directory %q: %w", dir, err)
	}
	return &DiskCache{dir: dir}, nil
}

func (c *DiskCache) keyPath(key string) string {
	hash := sha256.Sum256([]byte(key))
	filename := hex.EncodeToString(hash[:]) + ".cache"
	return filepath.Join(c.dir, filename)
}

// Get retrieves data for key. Returns false if not found or expired.
func (c *DiskCache) Get(key string) ([]byte, bool, error) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	path := c.keyPath(key)
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, false, nil
		}
		return nil, false, err
	}

	if len(data) < 8 {
		_ = os.Remove(path)
		return nil, false, nil
	}

	expMillis := int64(binary.BigEndian.Uint64(data[:8]))
	if expMillis > 0 && time.Now().UnixMilli() > expMillis {
		// Expired entry
		go func(p string) {
			c.mu.Lock()
			defer c.mu.Unlock()
			_ = os.Remove(p)
		}(path)
		return nil, false, nil
	}

	payload := make([]byte, len(data)-8)
	copy(payload, data[8:])
	return payload, true, nil
}

// Set stores data for key with the given TTL. If ttl <= 0, item never expires.
func (c *DiskCache) Set(key string, data []byte, ttl time.Duration) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	var expMillis int64
	if ttl > 0 {
		expMillis = time.Now().Add(ttl).UnixMilli()
	}

	buf := make([]byte, 8+len(data))
	binary.BigEndian.PutUint64(buf[:8], uint64(expMillis))
	copy(buf[8:], data)

	path := c.keyPath(key)
	tmpPath := fmt.Sprintf("%s.%d.tmp", path, time.Now().UnixNano())
	if err := os.WriteFile(tmpPath, buf, 0644); err != nil {
		return err
	}
	return os.Rename(tmpPath, path)
}

// Delete removes a key from cache.
func (c *DiskCache) Delete(key string) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	path := c.keyPath(key)
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return nil
}

// Clear flushes all entries in the cache.
func (c *DiskCache) Clear() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	entries, err := os.ReadDir(c.dir)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".cache" {
			_ = os.Remove(filepath.Join(c.dir, entry.Name()))
		}
	}
	return nil
}
