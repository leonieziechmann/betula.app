package store

import (
	"fmt"
	"net/url"
	"strconv"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

// TimeFormat is how the index keeps a time: UTC with microseconds, fixed width, so that the
// order of the strings is the order of the times. It is also RFC 3339, the form the API emits.
const TimeFormat = "2006-01-02T15:04:05.000000Z"

// FormatTime returns t in TimeFormat.
func FormatTime(t time.Time) string {
	return t.UTC().Format(TimeFormat)
}

// stamp is a time as the index keeps it: UTC, cut to the microsecond.
func stamp(t time.Time) time.Time {
	return t.UTC().Truncate(time.Microsecond)
}

func parseTime(s string) (time.Time, error) {
	t, err := time.Parse(TimeFormat, s)
	if err != nil {
		return time.Time{}, fmt.Errorf("invalid time %q in the index: %w", s, err)
	}
	return t, nil
}

func parseNullTime(s *string) (time.Time, error) {
	if s == nil {
		return time.Time{}, nil
	}
	return parseTime(*s)
}

// Canonical returns the key under which a request is stored, the normalized URL and the host
// (lower case, without port).
//
// Only http and https; scheme and host lower case; the default port (:80, :443) dropped; the
// fragment dropped; an empty path becomes "/". Path and query are kept byte for byte as sent
// (no decoding, no sorting): QIS's parameter order is part of its URLs. A URL with user info is
// refused. The key is "GET <normalizedURL>", followed by "\naccept: <accept>" and
// "\naccept-language: <acceptLanguage>" when they are not empty.
//
// Text that is not UTF-8 is refused, in the URL and in the headers: encoding/json would carry
// it to a follower as U+FFFD, so that the follower's key would be another (review 1). So is a
// host that is not ASCII: such a name has an ASCII spelling (punycode, xn--…), the one DNS
// knows, and two spellings of one host would be two keys.
func Canonical(rawURL, accept, acceptLanguage string) (key, normalizedURL, host string, err error) {
	if !utf8.ValidString(rawURL) {
		return "", "", "", fmt.Errorf("invalid url %q: not UTF-8", rawURL)
	}
	u, err := url.Parse(rawURL)
	if err != nil {
		return "", "", "", fmt.Errorf("invalid url: %w", err)
	}
	if u.Scheme != "http" && u.Scheme != "https" {
		return "", "", "", fmt.Errorf("invalid url %q: only http and https", rawURL)
	}
	if u.Opaque != "" || u.Host == "" {
		return "", "", "", fmt.Errorf("invalid url %q: no host", rawURL)
	}
	if u.User != nil {
		return "", "", "", fmt.Errorf("invalid url %q: user info is not allowed", rawURL)
	}
	host = strings.ToLower(u.Hostname())
	if host == "" {
		return "", "", "", fmt.Errorf("invalid url %q: no host", rawURL)
	}
	if !isASCII(host) {
		return "", "", "", fmt.Errorf("invalid url %q: the host is not ASCII (its punycode form, xn--…, is)", rawURL)
	}
	authority := host
	if strings.Contains(host, ":") {
		authority = "[" + host + "]"
	}
	if p := u.Port(); p != "" {
		port, err := strconv.Atoi(p)
		if err != nil || port <= 0 || port > 65535 {
			return "", "", "", fmt.Errorf("invalid url %q: invalid port", rawURL)
		}
		if !(u.Scheme == "http" && port == 80) && !(u.Scheme == "https" && port == 443) {
			authority += ":" + strconv.Itoa(port)
		}
	}

	// The raw text after the authority, which url.Parse ends at the first '/', '?' or '#'.
	_, afterSlashes, ok := strings.Cut(rawURL, "//")
	if !ok {
		return "", "", "", fmt.Errorf("invalid url %q: no host", rawURL)
	}
	rest := ""
	if i := strings.IndexAny(afterSlashes, "/?#"); i >= 0 {
		rest = afterSlashes[i:]
	}
	rest, _, _ = strings.Cut(rest, "#")
	if rest == "" || rest[0] == '?' {
		rest = "/" + rest
	}
	normalizedURL = u.Scheme + "://" + authority + rest

	for _, v := range []string{accept, acceptLanguage} {
		if !validHeaderText(v) {
			return "", "", "", fmt.Errorf("invalid header value %q", v)
		}
	}
	key = "GET " + normalizedURL
	if accept != "" {
		key += "\naccept: " + accept
	}
	if acceptLanguage != "" {
		key += "\naccept-language: " + acceptLanguage
	}
	return key, normalizedURL, host, nil
}

// validHeaderText says whether v can be a header value the index keeps: UTF-8 without control
// characters (a tab allowed), so that no newline forges a key and JSON carries it as it is.
func validHeaderText(v string) bool {
	return utf8.ValidString(v) && !strings.ContainsFunc(v, func(r rune) bool { return (r < 0x20 && r != '\t') || r == 0x7f })
}

func isASCII(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= utf8.RuneSelf {
			return false
		}
	}
	return true
}

// validSource says whether s is a source as the API takes it: 1 to 64 of a-z, 0-9, '_', '.', '-'.
func validSource(s string) bool {
	if s == "" || len(s) > 64 {
		return false
	}
	for i := 0; i < len(s); i++ {
		if c := s[i]; !('a' <= c && c <= 'z' || '0' <= c && c <= '9' || c == '_' || c == '.' || c == '-') {
			return false
		}
	}
	return true
}

// ValidHash says whether s is a sha256 as the store names blobs: 64 lower-case hex digits.
func ValidHash(s string) bool {
	if len(s) != 64 {
		return false
	}
	for i := 0; i < len(s); i++ {
		if !isHexDigit(s[i]) {
			return false
		}
	}
	return true
}

func isHexDigit(c byte) bool {
	return '0' <= c && c <= '9' || 'a' <= c && c <= 'f'
}

// ValidFileName says whether name can name a file: 1 to 1024 bytes of UTF-8, segments
// separated by '/', none of them empty, "." or "..", no control characters (so no leading or
// trailing '/' either).
func ValidFileName(name string) bool {
	if name == "" || len(name) > 1024 || !utf8.ValidString(name) {
		return false
	}
	if strings.ContainsFunc(name, unicode.IsControl) {
		return false
	}
	for _, seg := range strings.Split(name, "/") {
		if seg == "" || seg == "." || seg == ".." {
			return false
		}
	}
	return true
}
