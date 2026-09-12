package cache

import (
	"errors"
	"time"
)

var (
	// ErrExpired is returned when a cache entry exists but is past its TTL.
	ErrExpired = errors.New("cache entry expired")
	// ErrNotFound is returned when a cache key does not exist.
	ErrNotFound = errors.New("cache entry not found")
)

// Cache defines the contract for caching raw responses or scraped content.
type Cache interface {
	// Get retrieves data for key. Returns false if not found or expired.
	Get(key string) ([]byte, bool, error)
	// Set stores data for key with the given TTL. If ttl <= 0, item never expires.
	Set(key string, data []byte, ttl time.Duration) error
	// Delete removes a key from cache.
	Delete(key string) error
	// Clear flushes all entries in the cache.
	Clear() error
}
