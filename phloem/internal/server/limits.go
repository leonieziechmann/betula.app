package server

import (
	"sync"
	"time"
)

// limiter holds token buckets by key: rate tokens a second, at most burst, and a key it has not
// seen starts full. prune forgets the keys whose bucket is full again, so that it holds the keys
// of the last moments only; at maxKeys, the keys it has not seen share one bucket.
type limiter struct {
	rate, burst float64

	mu   sync.Mutex
	keys map[string]*bucket
}

type bucket struct {
	tokens float64
	at     time.Time
}

// maxKeys bounds a limiter's memory, whatever the number of addresses or spaces asking.
const maxKeys = 100_000

// overflowKey is the bucket the keys share once a limiter holds maxKeys.
const overflowKey = "\x00overflow"

// perWindow is a limiter of n a window, all of them at once at most.
func perWindow(n int, window time.Duration) *limiter {
	return newLimiter(float64(n)/window.Seconds(), float64(n))
}

func newLimiter(rate, burst float64) *limiter {
	return &limiter{rate: rate, burst: burst, keys: make(map[string]*bucket)}
}

func (l *limiter) fill(b *bucket, now time.Time) {
	if now.After(b.at) {
		b.tokens = min(l.burst, b.tokens+now.Sub(b.at).Seconds()*l.rate)
		b.at = now
	}
}

// take takes cost tokens of key's bucket if it has them (a cost above the burst costs the burst).
func (l *limiter) take(key string, cost float64, now time.Time) bool {
	cost = min(cost, l.burst)
	l.mu.Lock()
	defer l.mu.Unlock()
	b := l.keys[key]
	if b == nil {
		if len(l.keys) >= maxKeys {
			key = overflowKey
			b = l.keys[key]
		}
		if b == nil {
			b = &bucket{tokens: l.burst, at: now}
			l.keys[key] = b
		}
	}
	l.fill(b, now)
	if b.tokens < cost {
		return false
	}
	b.tokens -= cost
	return true
}

// give returns tokens taken for what did not happen.
func (l *limiter) give(key string, n float64) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if b := l.keys[key]; b != nil {
		b.tokens = min(l.burst, b.tokens+n)
	}
}

// prune forgets the buckets that are full again.
func (l *limiter) prune(now time.Time) {
	l.mu.Lock()
	defer l.mu.Unlock()
	for key, b := range l.keys {
		l.fill(b, now)
		if b.tokens >= l.burst {
			delete(l.keys, key)
		}
	}
}

// size is the number of keys held.
func (l *limiter) size() int {
	l.mu.Lock()
	defer l.mu.Unlock()
	return len(l.keys)
}
