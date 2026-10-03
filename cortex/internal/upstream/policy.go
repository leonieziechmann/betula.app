package upstream

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"mime"
	"net"
	"net/netip"
	"os"
	"sort"
	"strings"
	"time"
)

// DefaultUserAgent is what Cortex sends upstream when neither the client nor the policy names
// a User-Agent.
const DefaultUserAgent = "Cortex/1.0 (+https://betula.app; info@betula.app)"

// defaults are the settings of a host that neither the policy's "default" nor the host's own
// entry sets.
var defaults = HostPolicy{
	Concurrency:     1,
	Pause:           0,
	MaxAge:          24 * time.Hour,
	QueueWait:       60 * time.Second,
	BreakerFailures: 10,
	BreakerPause:    15 * time.Minute,
	Timeout:         10 * time.Minute,
	MaxBody:         8 << 30,
}

// builtinPolicy is the policy without a file (CORTEX_HOSTS_FILE empty): every public host with
// the defaults above, and a pause of their own for the hosts Betula's Radix reads.
const builtinPolicy = `{
  "allow": ["*"],
  "user_agent": "` + DefaultUserAgent + `",
  "hosts": {
    "qis.b-tu.de":   {"pause": "500ms"},
    "www.b-tu.de":   {"pause": "500ms"},
    "opus4.kobv.de": {"pause": "2s", "max_age": "720h", "expect_type": "application/pdf"}
  }
}`

// maxPolicyBytes bounds what LoadPolicy reads: a policy is a few kilobytes.
const maxPolicyBytes = 1 << 20

// maxBodyLimit is the largest max_body a policy may set (1 PiB): far above any disk Cortex
// has, and far enough below the largest int64 that counting bytes against it cannot overflow.
// There is no "unlimited", and the largest int64 is not a way to write it.
const maxBodyLimit = 1 << 50

// Policy says which hosts Cortex may fetch from and the floor each one gets. A Policy that
// New or ParsePolicy returned is shared and must not be changed.
type Policy struct {
	// Allow lists the hosts Cortex may fetch from: exact names, "*.suffix" (every name below
	// suffix, not suffix itself) or "*" (every host). Redirects are held to it too.
	Allow []string
	// UserAgent is sent when the client names none.
	UserAgent string
	// Default applies to every host without an entry in Hosts.
	Default HostPolicy
	// Hosts are the entries of single hosts (an exact name) or of every name below a suffix
	// ("*.suffix"), in lower case and with every field set (ParsePolicy fills in what the
	// file leaves out from Default).
	Hosts map[string]HostPolicy
}

// HostPolicy is the floor and the limits of one host. Through redirects, the answer at the end
// of the chain is held to the ExpectType of the host asked first as well as its own, and to the
// smaller MaxBody of the two.
type HostPolicy struct {
	// Name is the entry that applies: the host's own name, the "*.suffix" that matched, or ""
	// for the default. It is the host label of the metrics, "other" for "".
	Name string

	Concurrency     int           // requests in flight at most, 1 to 1000
	Pause           time.Duration // after each request (±30 %) before its slot is used again, up to 1 h
	MaxAge          time.Duration // how old a stored answer may be in mode cache (the server applies it)
	QueueWait       time.Duration // a request not started within this fails with *BusyError, up to 1 h
	BreakerFailures int           // failures in a row (network error, timeout, 5xx) that pause the host
	BreakerPause    time.Duration // how long they pause it, up to 24 h
	Timeout         time.Duration // the whole request, from sending it to the end of the body, up to 24 h
	MaxBody         int64         // bytes of a body at most, decoded and as it came over the wire (ErrTooLarge), up to 1 PiB
	ExpectType      string        // the media type a 2xx must have (ErrWrongType); "" takes any
}

// rawPolicy is the file as written: a field that is absent is nil and takes the default.
type rawPolicy struct {
	Allow     *[]string          `json:"allow"`
	UserAgent *string            `json:"user_agent"`
	Default   rawHost            `json:"default"`
	Hosts     map[string]rawHost `json:"hosts"`
}

type rawHost struct {
	Concurrency     *int    `json:"concurrency"`
	Pause           *string `json:"pause"`
	MaxAge          *string `json:"max_age"`
	QueueWait       *string `json:"queue_wait"`
	BreakerFailures *int    `json:"breaker_failures"`
	BreakerPause    *string `json:"breaker_pause"`
	Timeout         *string `json:"timeout"`
	MaxBody         *int64  `json:"max_body"`
	ExpectType      *string `json:"expect_type"`
}

// DefaultPolicy is the policy without a file: every public host may be fetched, with the
// defaults; qis.b-tu.de and www.b-tu.de pause 500 ms, opus4.kobv.de 2 s and keeps only PDFs.
func DefaultPolicy() *Policy {
	p, err := ParsePolicy([]byte(builtinPolicy))
	if err != nil {
		panic("upstream: the built-in policy is invalid: " + err.Error())
	}
	return p
}

// ParsePolicy reads a policy file (JSON). Durations are Go durations ("500ms", "24h"). A field
// the file leaves out takes the default: "allow" every host, "user_agent" DefaultUserAgent, a
// field of "default" the built-in default, a field of a host's entry the "default" of the file.
// The file's "hosts" replace the built-in ones. Unknown fields are an error, so a misspelt
// setting does not go unnoticed.
func ParsePolicy(data []byte) (*Policy, error) {
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	var raw rawPolicy
	if err := dec.Decode(&raw); err != nil {
		return nil, fmt.Errorf("policy: %w", err)
	}
	if _, err := dec.Token(); err != io.EOF {
		return nil, errors.New("policy: data after the JSON object")
	}

	p := &Policy{Allow: []string{"*"}, UserAgent: DefaultUserAgent, Default: defaults}
	if raw.Allow != nil {
		p.Allow = *raw.Allow
	}
	if raw.UserAgent != nil {
		p.UserAgent = *raw.UserAgent
	}
	if err := raw.Default.apply(&p.Default); err != nil {
		return nil, fmt.Errorf("policy: default: %w", err)
	}
	p.Hosts = make(map[string]HostPolicy, len(raw.Hosts))
	for name, rh := range raw.Hosts {
		hp := p.Default
		if err := rh.apply(&hp); err != nil {
			return nil, fmt.Errorf("policy: hosts[%q]: %w", name, err)
		}
		p.Hosts[name] = hp
	}
	return p.normalised()
}

// LoadPolicy reads and parses the policy file at path.
func LoadPolicy(path string) (*Policy, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, maxPolicyBytes+1))
	if err != nil {
		return nil, fmt.Errorf("failed to read %s: %w", path, err)
	}
	if len(data) > maxPolicyBytes {
		return nil, fmt.Errorf("%s: larger than %d bytes", path, maxPolicyBytes)
	}
	p, err := ParsePolicy(data)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return p, nil
}

func (rh rawHost) apply(hp *HostPolicy) error {
	if rh.Concurrency != nil {
		hp.Concurrency = *rh.Concurrency
	}
	if rh.BreakerFailures != nil {
		hp.BreakerFailures = *rh.BreakerFailures
	}
	if rh.MaxBody != nil {
		hp.MaxBody = *rh.MaxBody
	}
	if rh.ExpectType != nil {
		hp.ExpectType = *rh.ExpectType
	}
	for _, d := range []struct {
		name string
		raw  *string
		dst  *time.Duration
	}{
		{"pause", rh.Pause, &hp.Pause},
		{"max_age", rh.MaxAge, &hp.MaxAge},
		{"queue_wait", rh.QueueWait, &hp.QueueWait},
		{"breaker_pause", rh.BreakerPause, &hp.BreakerPause},
		{"timeout", rh.Timeout, &hp.Timeout},
	} {
		if d.raw == nil {
			continue
		}
		v, err := time.ParseDuration(*d.raw)
		if err != nil {
			return fmt.Errorf("%s: %w", d.name, err)
		}
		*d.dst = v
	}
	return nil
}

// normalised checks p and returns a copy with the names in lower case and the Name of each
// entry set.
func (p *Policy) normalised() (*Policy, error) {
	if p.UserAgent == "" || strings.ContainsFunc(p.UserAgent, isControl) {
		return nil, fmt.Errorf("policy: user_agent %q: empty or with control characters", p.UserAgent)
	}
	out := &Policy{
		Allow:     make([]string, 0, len(p.Allow)),
		UserAgent: p.UserAgent,
		Default:   p.Default,
		Hosts:     make(map[string]HostPolicy, len(p.Hosts)),
	}
	for _, a := range p.Allow {
		key := normalKey(a)
		if key != "*" && !validPattern(key) {
			return nil, fmt.Errorf("policy: allow: %q is not a host name, \"*.suffix\" or \"*\"", a)
		}
		out.Allow = append(out.Allow, key)
	}
	out.Default.Name = ""
	if err := out.Default.check(); err != nil {
		return nil, fmt.Errorf("policy: default: %w", err)
	}
	for name, hp := range p.Hosts {
		key := normalKey(name)
		if !validPattern(key) {
			return nil, fmt.Errorf("policy: hosts: %q is not a host name or \"*.suffix\"", name)
		}
		if _, dup := out.Hosts[key]; dup {
			return nil, fmt.Errorf("policy: hosts: %q twice", key)
		}
		hp.Name = key
		if err := hp.check(); err != nil {
			return nil, fmt.Errorf("policy: hosts[%q]: %w", name, err)
		}
		out.Hosts[key] = hp
	}
	return out, nil
}

// check validates the settings of one host and puts its media type in lower case.
func (hp *HostPolicy) check() error {
	switch {
	case hp.Concurrency < 1 || hp.Concurrency > 1000:
		return fmt.Errorf("concurrency %d: from 1 to 1000", hp.Concurrency)
	case hp.Pause < 0 || hp.Pause > time.Hour:
		return fmt.Errorf("pause %s: from 0 to 1h", hp.Pause)
	case hp.MaxAge < 0:
		return fmt.Errorf("max_age %s: negative", hp.MaxAge)
	case hp.QueueWait <= 0 || hp.QueueWait > time.Hour:
		return fmt.Errorf("queue_wait %s: more than 0, up to 1h", hp.QueueWait)
	case hp.BreakerFailures < 1:
		return fmt.Errorf("breaker_failures %d: at least 1", hp.BreakerFailures)
	case hp.BreakerPause <= 0 || hp.BreakerPause > 24*time.Hour:
		return fmt.Errorf("breaker_pause %s: more than 0, up to 24h", hp.BreakerPause)
	case hp.Timeout <= 0 || hp.Timeout > 24*time.Hour:
		return fmt.Errorf("timeout %s: more than 0, up to 24h", hp.Timeout)
	case hp.MaxBody < 1 || hp.MaxBody > maxBodyLimit:
		return fmt.Errorf("max_body %d: from 1 to %d (1 PiB)", hp.MaxBody, int64(maxBodyLimit))
	}
	if hp.ExpectType != "" {
		mt, params, err := mime.ParseMediaType(hp.ExpectType)
		if err != nil || len(params) > 0 || !strings.Contains(mt, "/") {
			return fmt.Errorf("expect_type %q: not a media type without parameters", hp.ExpectType)
		}
		hp.ExpectType = mt
	}
	return nil
}

// For returns the settings of host (a host name, with or without a port): its own entry,
// else the longest "*.suffix" entry it is below, else the default.
func (p *Policy) For(host string) HostPolicy {
	host = normalHost(host)
	if hp, ok := p.Hosts[host]; ok && !strings.HasPrefix(host, "*.") {
		return hp
	}
	best := ""
	for key := range p.Hosts {
		if strings.HasPrefix(key, "*.") && strings.HasSuffix(host, key[1:]) && len(key) > len(best) {
			best = key
		}
	}
	if best != "" {
		return p.Hosts[best]
	}
	return p.Default
}

// Allows says whether Cortex may fetch from host (a host name, with or without a port).
func (p *Policy) Allows(host string) bool {
	host = normalHost(host)
	if host == "" {
		return false
	}
	for _, a := range p.Allow {
		if a == "*" || a == host || (strings.HasPrefix(a, "*.") && strings.HasSuffix(host, a[1:])) {
			return true
		}
	}
	return false
}

// labels are the host labels of the metrics under p: every entry and "other", sorted.
func (p *Policy) labels() []string {
	labels := make([]string, 0, len(p.Hosts)+1)
	for key := range p.Hosts {
		labels = append(labels, key)
	}
	sort.Strings(labels)
	return append(labels, otherLabel)
}

// otherLabel is the host label of the metrics for every host without an entry of its own.
const otherLabel = "other"

func (hp HostPolicy) label() string {
	if hp.Name == "" {
		return otherLabel
	}
	return hp.Name
}

// normalHost turns the host of a URL into the form the policy and the host states use: lower
// case, without port, brackets or a trailing dot.
func normalHost(host string) string {
	host = strings.ToLower(host)
	if h, _, err := net.SplitHostPort(host); err == nil {
		host = h
	} else if strings.HasPrefix(host, "[") && strings.HasSuffix(host, "]") {
		host = host[1 : len(host)-1]
	}
	return strings.TrimSuffix(host, ".")
}

// normalKey is normalHost for the names in a policy, which have no port.
func normalKey(name string) string {
	return strings.TrimSuffix(strings.ToLower(name), ".")
}

// validPattern accepts a host name, an IP address or "*.name".
func validPattern(s string) bool {
	if name, ok := strings.CutPrefix(s, "*."); ok {
		return validName(name)
	}
	return validName(s)
}

func validName(s string) bool {
	if ip, err := netip.ParseAddr(s); err == nil {
		return ip.Zone() == ""
	}
	if s == "" || len(s) > 253 {
		return false
	}
	for _, label := range strings.Split(s, ".") {
		if label == "" || len(label) > 63 {
			return false
		}
		for _, c := range label {
			if !(c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '-' || c == '_') {
				return false
			}
		}
	}
	return true
}

func isControl(r rune) bool { return r < 0x20 || r == 0x7f }
