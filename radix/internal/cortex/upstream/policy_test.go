package upstream

import (
	"context"
	"io"
	"math"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestDefaultPolicyIsTheBuiltInOne(t *testing.T) {
	p := DefaultPolicy()
	if strings.Join(p.Allow, ",") != "*" || p.UserAgent != "Cortex/1.0 (+https://betula.app; info@betula.app)" {
		t.Errorf("allow %v, user agent %q", p.Allow, p.UserAgent)
	}
	want := HostPolicy{Concurrency: 1, Pause: 0, MaxAge: 24 * time.Hour, QueueWait: 60 * time.Second, BreakerFailures: 10,
		BreakerPause: 15 * time.Minute, Timeout: 10 * time.Minute, MaxBody: 8589934592}
	if p.Default != want {
		t.Errorf("default %+v, want %+v", p.Default, want)
	}
	qis := want
	qis.Name, qis.Pause = "qis.b-tu.de", 500*time.Millisecond
	opus := want
	opus.Name, opus.Pause, opus.MaxAge, opus.ExpectType = "opus4.kobv.de", 2*time.Second, 720*time.Hour, "application/pdf"
	if len(p.Hosts) != 3 || p.For("qis.b-tu.de") != qis || p.For("www.b-tu.de").Pause != 500*time.Millisecond || p.For("opus4.kobv.de") != opus {
		t.Errorf("hosts %+v", p.Hosts)
	}
	if !p.Allows("example.org") || p.For("example.org").label() != "other" {
		t.Errorf("an unconfigured host is not allowed with the defaults")
	}
}

func TestParsePolicyFillsInWhatTheFileLeavesOut(t *testing.T) {
	p, err := ParsePolicy([]byte(`{
		"default": {"pause": "1s", "concurrency": 2},
		"hosts": {
			"Example.ORG.": {"max_age": "1h"},
			"*.cdn.example.net": {"concurrency": 4, "pause": "0s", "expect_type": "Image/PNG"}
		}
	}`))
	if err != nil {
		t.Fatalf("ParsePolicy failed: %v", err)
	}
	if strings.Join(p.Allow, ",") != "*" || p.UserAgent != DefaultUserAgent {
		t.Errorf("allow %v, user agent %q: want the defaults", p.Allow, p.UserAgent)
	}
	if p.Default.Pause != time.Second || p.Default.Concurrency != 2 || p.Default.MaxAge != 24*time.Hour || p.Default.BreakerFailures != 10 {
		t.Errorf("default %+v, want the file's fields over the built-in ones", p.Default)
	}
	if hp := p.For("example.org"); hp.Name != "example.org" || hp.MaxAge != time.Hour || hp.Pause != time.Second || hp.Concurrency != 2 || hp.QueueWait != time.Minute {
		t.Errorf("example.org %+v, want its max_age over the file's default", hp)
	}
	if hp := p.For("a.cdn.example.net"); hp.Name != "*.cdn.example.net" || hp.Concurrency != 4 || hp.Pause != 0 || hp.ExpectType != "image/png" {
		t.Errorf("a.cdn.example.net %+v", hp)
	}
	if len(p.Hosts) != 2 {
		t.Errorf("hosts %v, want the file's two: they replace the built-in ones", p.Hosts)
	}
}

func TestPolicyForTakesTheExactNameThenTheLongestSuffixThenTheDefault(t *testing.T) {
	p, err := ParsePolicy([]byte(`{"hosts": {
		"example.org": {"pause": "1s"},
		"*.example.org": {"pause": "2s"},
		"*.static.example.org": {"pause": "3s"},
		"files.static.example.org": {"pause": "4s"},
		"10.0.0.1": {"pause": "5s"},
		"2001:db8::1": {"pause": "6s"}
	}}`))
	if err != nil {
		t.Fatalf("ParsePolicy failed: %v", err)
	}
	for _, tc := range []struct {
		host, name string
		pause      time.Duration
	}{
		{"example.org", "example.org", time.Second},
		{"EXAMPLE.org:8443", "example.org", time.Second},
		{"example.org.", "example.org", time.Second},
		{"www.example.org", "*.example.org", 2 * time.Second},
		{"a.b.example.org", "*.example.org", 2 * time.Second},
		{"img.static.example.org", "*.static.example.org", 3 * time.Second},
		{"files.static.example.org", "files.static.example.org", 4 * time.Second},
		{"static.example.org", "*.example.org", 2 * time.Second},
		{"badexample.org", "", 0},
		{"example.org.evil.net", "", 0},
		{"10.0.0.1", "10.0.0.1", 5 * time.Second},
		{"[2001:db8::1]:443", "2001:db8::1", 6 * time.Second},
		{"[2001:db8::1]", "2001:db8::1", 6 * time.Second},
		{"*.example.org", "*.example.org", 2 * time.Second},
	} {
		if hp := p.For(tc.host); hp.Name != tc.name || hp.Pause != tc.pause {
			t.Errorf("For(%q) = %q with pause %s, want %q with %s", tc.host, hp.Name, hp.Pause, tc.name, tc.pause)
		}
	}
	if got := strings.Join(p.labels(), " "); got != "*.example.org *.static.example.org 10.0.0.1 2001:db8::1 example.org files.static.example.org other" {
		t.Errorf("labels %q", got)
	}
}

func TestPolicyAllows(t *testing.T) {
	p, err := ParsePolicy([]byte(`{"allow": ["Example.org", "*.cdn.net", "10.0.0.1"]}`))
	if err != nil {
		t.Fatalf("ParsePolicy failed: %v", err)
	}
	for _, tc := range []struct {
		host string
		want bool
	}{
		{"example.org", true},
		{"EXAMPLE.ORG:443", true},
		{"www.example.org", false},
		{"a.cdn.net", true},
		{"x.a.cdn.net", true},
		{"cdn.net", false},
		{"evilcdn.net", false},
		{"10.0.0.1", true},
		{"10.0.0.2", false},
		{"", false},
	} {
		if got := p.Allows(tc.host); got != tc.want {
			t.Errorf("Allows(%q) = %v, want %v", tc.host, got, tc.want)
		}
	}
	none, err := ParsePolicy([]byte(`{"allow": []}`))
	if err != nil || none.Allows("example.org") {
		t.Errorf("an empty allow list allows example.org (err %v)", err)
	}
	all := DefaultPolicy()
	if !all.Allows("anything.example") {
		t.Errorf("\"*\" does not allow anything.example")
	}
}

func TestParsePolicyRejectsBrokenFiles(t *testing.T) {
	for _, tc := range []struct{ name, file string }{
		{"not JSON", `allow: ["*"]`},
		{"an array", `["*"]`},
		{"two objects", `{} {}`},
		{"an unknown field", `{"default": {"puase": "1s"}}`},
		{"an unknown top-level field", `{"host": {}}`},
		{"a bad duration", `{"default": {"pause": "fast"}}`},
		{"a duration as a number", `{"default": {"pause": 500}}`},
		{"a negative pause", `{"default": {"pause": "-1s"}}`},
		{"concurrency 0", `{"default": {"concurrency": 0}}`},
		{"a host's concurrency 0", `{"hosts": {"example.org": {"concurrency": 0}}}`},
		{"queue_wait 0", `{"default": {"queue_wait": "0s"}}`},
		{"queue_wait over an hour", `{"default": {"queue_wait": "2h"}}`},
		{"breaker_failures 0", `{"default": {"breaker_failures": 0}}`},
		{"timeout 0", `{"default": {"timeout": "0s"}}`},
		{"max_body 0", `{"default": {"max_body": 0}}`},
		{"max_body over 1 PiB", `{"default": {"max_body": 1125899906842625}}`},
		{"max_body the largest int64", `{"default": {"max_body": 9223372036854775807}}`},
		{"a host's max_body over 1 PiB", `{"hosts": {"example.org": {"max_body": 1125899906842625}}}`},
		{"expect_type without a slash", `{"default": {"expect_type": "pdf"}}`},
		{"expect_type with parameters", `{"default": {"expect_type": "text/html; charset=utf-8"}}`},
		{"a host \"*\"", `{"hosts": {"*": {}}}`},
		{"a host with a port", `{"hosts": {"example.org:8080": {}}}`},
		{"a host with a space", `{"hosts": {"exa mple.org": {}}}`},
		{"a host URL", `{"hosts": {"https://example.org": {}}}`},
		{"a host twice", `{"hosts": {"example.org": {}, "Example.org": {}}}`},
		{"an allow URL", `{"allow": ["http://example.org"]}`},
		{"an allow wildcard in the middle", `{"allow": ["www.*.org"]}`},
		{"an empty user agent", `{"user_agent": ""}`},
		{"a user agent with a line break", `{"user_agent": "Cortex\r\nX-Evil: 1"}`},
	} {
		if p, err := ParsePolicy([]byte(tc.file)); err == nil {
			t.Errorf("%s: parsed as %+v, want an error", tc.name, p)
		}
	}
}

func TestMaxBodyIsAtMostOnePebibyte(t *testing.T) {
	// The largest int64 is no way to write "unlimited": counting bytes up to it would overflow.
	p, err := ParsePolicy([]byte(`{"default": {"max_body": 1125899906842624}}`))
	if err != nil || p.Default.MaxBody != 1<<50 {
		t.Fatalf("max_body 1 PiB: got %+v (err %v), want it accepted", p, err)
	}
	huge := testPolicy(func(h *HostPolicy) { h.MaxBody = math.MaxInt64 })
	if u, err := New(Options{Policy: huge}); err == nil || !strings.Contains(err.Error(), "max_body") {
		if u != nil {
			u.Close()
		}
		t.Errorf("a Policy with max_body MaxInt64: err %v, want it refused", err)
	}
}

func TestNewRefusesAnInvalidOrIncompletePolicy(t *testing.T) {
	dir := t.TempDir()
	broken := filepath.Join(dir, "broken.json")
	if err := os.WriteFile(broken, []byte(`{"default": {"pause": "fast"}}`), 0644); err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name string
		opt  Options
	}{
		{"a broken file", Options{PolicyFile: broken}},
		{"a missing file", Options{PolicyFile: filepath.Join(dir, "missing.json")}},
		{"a policy and a file", Options{Policy: DefaultPolicy(), PolicyFile: broken}},
		{"a policy without its defaults", Options{Policy: &Policy{Allow: []string{"*"}, UserAgent: "x"}}},
	} {
		if u, err := New(tc.opt); err == nil {
			u.Close()
			t.Errorf("%s: New succeeded", tc.name)
		}
	}
}

// writePolicy writes a policy file and gives it a modification time of its own, so that the
// coarse clock of the file system cannot hide a change.
func writePolicy(t *testing.T, path, content string, mtime time.Time) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(path, mtime, mtime); err != nil {
		t.Fatal(err)
	}
}

func TestPolicyIsReloadedWhenTheFileChangesAndABrokenFileKeepsTheLastGoodOne(t *testing.T) {
	log := captureLog(t)
	path := filepath.Join(t.TempDir(), "hosts.json")
	mtime := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	writePolicy(t, path, `{"hosts": {"a.example": {"pause": "100ms"}}}`, mtime)
	u := openUpstream(t, Options{PolicyFile: path})
	pauseOf := func() time.Duration { return u.Policy().For("a.example").Pause }
	if pauseOf() != 100*time.Millisecond {
		t.Fatalf("pause %s, want the file's", pauseOf())
	}
	if recs := log.records(t, "policy.loaded"); len(recs) != 1 || recs[0]["file"] != path || recs[0]["level"] != "INFO" {
		t.Errorf("policy.loaded records %v, want one naming the file", recs)
	}

	first := u.Policy()
	u.checkPolicy()
	if u.Policy() != first {
		t.Error("an unchanged file was loaded again")
	}

	writePolicy(t, path, `{"hosts": {"a.example": {"pause": "200ms"}}}`, mtime.Add(time.Second))
	u.checkPolicy()
	if pauseOf() != 200*time.Millisecond {
		t.Errorf("pause %s after the change, want 200ms", pauseOf())
	}

	writePolicy(t, path, `{"hosts": {"a.example": {"pause": "soon"}}}`, mtime.Add(2*time.Second))
	u.checkPolicy()
	u.checkPolicy()
	if pauseOf() != 200*time.Millisecond {
		t.Errorf("pause %s after a broken file, want the last good 200ms", pauseOf())
	}
	recs := log.records(t, "policy.invalid")
	if len(recs) != 1 || recs[0]["level"] != "WARN" || !strings.Contains(recs[0]["error"].(string), "pause") {
		t.Errorf("policy.invalid records %v, want one WARN naming the field", recs)
	}

	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	u.checkPolicy()
	u.checkPolicy()
	if pauseOf() != 200*time.Millisecond {
		t.Errorf("pause %s without the file, want the last good 200ms", pauseOf())
	}
	if n := len(log.records(t, "policy.invalid")); n != 2 {
		t.Errorf("%d policy.invalid records, want one more for the missing file", n)
	}

	writePolicy(t, path, `{"hosts": {"a.example": {"pause": "300ms"}}}`, mtime.Add(3*time.Second))
	u.checkPolicy()
	if pauseOf() != 300*time.Millisecond {
		t.Errorf("pause %s after the fix, want 300ms", pauseOf())
	}
	if n := len(log.records(t, "policy.loaded")); n != 3 {
		t.Errorf("%d policy.loaded records, want 3", n)
	}
}

func TestARaisedConcurrencyLetsWaitingRequestsStart(t *testing.T) {
	g := newGate(t)
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) {
		g.wait(r)
		_, _ = io.WriteString(w, "ok")
	})
	path := filepath.Join(t.TempDir(), "hosts.json")
	mtime := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	writePolicy(t, path, `{"hosts": {"127.0.0.1": {"concurrency": 1}}}`, mtime)
	u := newTestUpstream(t, Options{PolicyFile: path})

	var wg sync.WaitGroup
	for i := 0; i < 2; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, _ = u.Fetch(context.Background(), Request{URL: f.srv.URL, Sink: discardSink})
		}()
	}
	waitFor(t, "one in flight and one waiting", func() bool {
		hs := stateOf(u, "127.0.0.1")
		return hs.InFlight == 1 && hs.Queue == 1
	})
	writePolicy(t, path, `{"hosts": {"127.0.0.1": {"concurrency": 2}}}`, mtime.Add(time.Second))
	u.checkPolicy()
	waitFor(t, "both in flight", func() bool { return f.running() == 2 })
	g.open()
	wg.Wait()
}

func TestTheWatcherReloadsTheFileUntilClose(t *testing.T) {
	path := filepath.Join(t.TempDir(), "hosts.json")
	mtime := time.Date(2026, 10, 2, 10, 0, 0, 0, time.UTC)
	writePolicy(t, path, `{"user_agent": "One/1.0"}`, mtime)
	u, err := New(Options{PolicyFile: path, proxy: noProxy, checkEvery: 5 * time.Millisecond})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}
	writePolicy(t, path, `{"user_agent": "Two/1.0"}`, mtime.Add(time.Second))
	waitFor(t, "the new policy", func() bool { return u.Policy().UserAgent == "Two/1.0" })

	u.Close()
	select {
	case <-u.done:
	default:
		t.Fatal("the watcher still runs after Close")
	}
	u.Close() // twice is harmless
	writePolicy(t, path, `{"user_agent": "Three/1.0"}`, mtime.Add(2*time.Second))
	time.Sleep(30 * time.Millisecond)
	if u.Policy().UserAgent != "Two/1.0" {
		t.Error("the policy changed after Close")
	}
}
