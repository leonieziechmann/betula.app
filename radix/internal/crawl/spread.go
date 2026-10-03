package crawl

import (
	"crypto/sha256"
	"encoding/binary"
	"time"
)

// Due reports whether a page read at fetched is due again at now when it is to be read
// once per period. Every key has a time of its own in the period, derived from the key
// alone, and the page is due once that time has come at least half a period after it was
// read: pages read together, a whole archive in the same few nights, spread evenly over
// the period instead of coming due together again, and a page read at another time, for
// another reason, waits for its time in the next period rather than catching up. In the
// steady state a page is read once per period; after a read out of turn, within half a
// period to one and a half.
func Due(key string, fetched, now time.Time, period time.Duration) bool {
	if period <= 0 || fetched.IsZero() {
		return true
	}
	// SHA-256 mixes every byte into every bit: module numbers differ in their last
	// digits, and a hash that leaves the high bits alike would give them one night.
	sum := sha256.Sum256([]byte(key))
	phase := int64(binary.BigEndian.Uint64(sum[:8]) % uint64(period))
	since := now.UnixNano() - phase
	slot := since - since%int64(period) + phase // the key's last time before now
	return slot >= fetched.UnixNano()+int64(period/2)
}

// fresh says whether an archived page is recent enough not to be fetched: younger than
// MaxAge, or with Spread, not Due within its period.
func (opt Options) fresh(key string, fetched time.Time) bool {
	if opt.MaxAge <= 0 || fetched.IsZero() {
		return false
	}
	if opt.Spread {
		return !Due(key, fetched, time.Now(), opt.MaxAge)
	}
	return time.Since(fetched) < opt.MaxAge
}
