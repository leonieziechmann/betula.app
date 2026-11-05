package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestCreateSpace(t *testing.T) {
	e := newEnv(t, nil)
	p, a := newProfile(), randBytes(16)
	if status, doc := e.create(p, a, ""); status != http.StatusCreated || doc["slot"] != float64(1) {
		t.Fatalf("create: %d %v", status, doc)
	}
	if status, doc := e.create(p, randBytes(16), ""); status != http.StatusConflict || doc["error"] != "exists" {
		t.Fatalf("again: %d %v", status, doc)
	}
	sp, err := e.st.Space(t.Context(), p.space)
	if err != nil || string(sp.TokenHash) == string(p.token) || len(sp.TokenHash) != 32 {
		t.Fatalf("the token must be kept as its hash: %+v %v", sp, err)
	}
	for name, body := range map[string]any{
		"short space": map[string]string{"space": b64s(randBytes(15)), "token": b64s(randBytes(32)), "device": b64s(a)},
		"long token":  map[string]string{"space": b64s(randBytes(16)), "token": b64s(randBytes(33)), "device": b64s(a)},
		"no device":   map[string]string{"space": b64s(randBytes(16)), "token": b64s(randBytes(32))},
		"std base64":  map[string]string{"space": "AAAAAAAAAAAAAAAAAAAAAA==", "token": b64s(randBytes(32)), "device": b64s(a)},
		"not json":    "space",
	} {
		if status, doc, _ := e.request(http.MethodPost, "/sync/v1/space", nil, body); status != http.StatusBadRequest || doc["error"] != "invalid" {
			t.Errorf("%s: %d %v", name, status, doc)
		}
	}
	if status, doc, _ := e.request(http.MethodPost, "/sync/v1/space", nil, `{"space":"`+strings.Repeat("A", 2000)+`"}`); status != http.StatusRequestEntityTooLarge {
		t.Errorf("a large body: %d %v", status, doc)
	}
}

func TestCreationIsLimitedByAddressAndInAll(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.SpacesPerAddr, o.Limits.SpacesTotal = 2, 3 })
	create := func(addr string) int {
		t.Helper()
		status, _ := e.create(newProfile(), randBytes(16), addr)
		return status
	}
	if got := []int{create("203.0.113.1"), create("203.0.113.1"), create("203.0.113.1")}; fmt.Sprint(got) != "[201 201 429]" {
		t.Fatalf("one address: %v", got)
	}
	if got := []int{create("203.0.113.2"), create("203.0.113.3")}; fmt.Sprint(got) != "[201 429]" {
		t.Fatalf("in all: %v", got)
	}
	// A space that exists already costs nothing.
	p := newProfile()
	e.clock.Advance(time.Hour)
	if status, _ := e.create(p, randBytes(16), "203.0.113.9"); status != http.StatusCreated {
		t.Fatalf("after an hour: %d", status)
	}
	for range 3 {
		if status, _ := e.create(p, randBytes(16), "203.0.113.9"); status != http.StatusConflict {
			t.Fatalf("exists: %d", status)
		}
	}
	if status, _ := e.create(newProfile(), randBytes(16), "203.0.113.9"); status != http.StatusCreated {
		t.Fatalf("the second of the address: %d", status)
	}
	// IPv6: one /64 is one address.
	e.clock.Advance(time.Hour)
	if got := []int{create("2001:db8:1:2::1"), create("2001:db8:1:2::ffff"), create("2001:db8:1:2:abcd::1")}; fmt.Sprint(got) != "[201 201 429]" {
		t.Fatalf("one /64: %v", got)
	}
}

func TestTheBudgetStopsNewSpacesAndParcels(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Budget = 300 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	c.push("1", strings.Repeat("x", 300)) // pushes go on: the old spaces go on
	c.expect("stored")
	if status, doc := e.create(newProfile(), randBytes(16), ""); status != http.StatusInsufficientStorage || doc["error"] != "budget" {
		t.Fatalf("a space beyond the budget: %d %v", status, doc)
	}
	status, doc, _ := e.request(http.MethodPost, "/sync/v1/pair", p.headers(), map[string]string{"id": b64s(randBytes(16)), "box": b64s([]byte("box"))})
	if status != http.StatusInsufficientStorage || doc["error"] != "budget" {
		t.Fatalf("a parcel beyond the budget: %d %v", status, doc)
	}
	c.push("2", "more")
	c.expect("stored")
	if st := e.srv.Status(); st.BudgetUsed < 1 || st.Budget != 300 {
		t.Errorf("status %+v", st)
	}
}

func TestParcelsAreFetchedOnce(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.MaxPairs, o.Limits.MaxBox = 2, 100 })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	leave := func(hdr map[string]string, id []byte, box []byte) (int, map[string]any) {
		t.Helper()
		status, doc, _ := e.request(http.MethodPost, "/sync/v1/pair", hdr, map[string]string{"id": b64s(id), "box": b64s(box)})
		return status, doc
	}
	take := func(id string) (int, map[string]any) {
		t.Helper()
		status, doc, _ := e.request(http.MethodGet, "/sync/v1/pair", map[string]string{"Betula-Pair": id}, nil)
		return status, doc
	}
	one := randBytes(16)
	if status, doc := leave(p.headers(), one, []byte("the keys, sealed")); status != http.StatusCreated {
		t.Fatalf("leave: %d %v", status, doc)
	}
	if status, doc := leave(p.headers(), one, []byte("again")); status != http.StatusConflict || doc["error"] != "exists" {
		t.Fatalf("the same id: %d %v", status, doc)
	}
	if status, doc := take(b64s(one)); status != http.StatusOK || decoded(t, doc["box"].(string)) != "the keys, sealed" {
		t.Fatalf("take: %d %v", status, doc)
	}
	if status, doc := take(b64s(one)); status != http.StatusNotFound || doc["error"] != "not_found" {
		t.Fatalf("a second take: %d %v", status, doc)
	}
	two := randBytes(16)
	leave(p.headers(), two, []byte("x"))
	e.clock.Advance(10*time.Minute + time.Second)
	if status, _ := take(b64s(two)); status != http.StatusNotFound {
		t.Fatalf("an expired parcel: %d", status)
	}
	for range 2 {
		if status, _ := leave(p.headers(), randBytes(16), []byte("x")); status != http.StatusCreated {
			t.Fatalf("leave: %d", status)
		}
	}
	if status, doc := leave(p.headers(), randBytes(16), []byte("x")); status != http.StatusTooManyRequests || doc["error"] != "slow_down" {
		t.Fatalf("a third parcel waiting: %d %v", status, doc)
	}
	if status, doc := leave(p.headers(), randBytes(16), make([]byte, 101)); status != http.StatusRequestEntityTooLarge || doc["error"] != "too_big" {
		t.Fatalf("a large box: %d %v", status, doc)
	}
	wrong := profile{p.space, randBytes(32)}
	if status, doc := leave(wrong.headers(), randBytes(16), []byte("x")); status != http.StatusUnauthorized || doc["error"] != "unauthorized" {
		t.Fatalf("a wrong token: %d %v", status, doc)
	}
	if status, _ := leave(map[string]string{"Betula-Space": b64s(p.space)}, randBytes(16), []byte("x")); status != http.StatusUnauthorized {
		t.Fatalf("no token: %d", status)
	}
	if status, _ := take("short"); status != http.StatusBadRequest {
		t.Fatalf("a malformed id: %d", status)
	}
	// A parcel touches the space.
	e.clock.Advance(394 * 24 * time.Hour)
	leave(p.headers(), randBytes(16), []byte("x"))
	e.clock.Advance(2 * 24 * time.Hour)
	e.srv.sweep()
	if _, err := e.st.Space(t.Context(), p.space); err != nil {
		t.Fatalf("a space with a parcel 2 days ago expired: %v", err)
	}
}

func TestTakesAreLimitedByAddress(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.TakesPerAddr = 2 })
	hdr := map[string]string{"Betula-Pair": b64s(randBytes(16)), "X-Real-Ip": "198.51.100.7"}
	var got []int
	for range 3 {
		status, _, _ := e.request(http.MethodGet, "/sync/v1/pair", hdr, nil)
		got = append(got, status)
	}
	if fmt.Sprint(got) != "[404 404 429]" {
		t.Fatalf("takes of one address: %v", got)
	}
}

func TestOriginsAndCORS(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Origins = []string{"https://betula.app", "http://localhost:8080"} })
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	body := map[string]string{"space": b64s(randBytes(16)), "token": b64s(randBytes(32)), "device": b64s(a)}
	status, doc, _ := e.request(http.MethodPost, "/sync/v1/space", map[string]string{"Origin": "https://evil.example"}, body)
	if status != http.StatusForbidden || doc["error"] != "origin" {
		t.Fatalf("another origin: %d %v", status, doc)
	}
	status, _, hdr := e.request(http.MethodPost, "/sync/v1/space", map[string]string{"Origin": "http://localhost:8080"}, body)
	if status != http.StatusCreated || hdr.Get("Access-Control-Allow-Origin") != "http://localhost:8080" || !strings.Contains(hdr.Get("Vary"), "Origin") {
		t.Fatalf("an origin taken: %d %v", status, hdr)
	}
	status, _, hdr = e.request(http.MethodOptions, "/sync/v1/pair", map[string]string{
		"Origin": "https://betula.app", "Access-Control-Request-Method": "POST", "Access-Control-Request-Headers": "authorization, betula-space"}, nil)
	if status != http.StatusNoContent || hdr.Get("Access-Control-Allow-Origin") != "https://betula.app" ||
		!strings.Contains(hdr.Get("Access-Control-Allow-Headers"), "Betula-Space") || !strings.Contains(hdr.Get("Access-Control-Allow-Methods"), "DELETE") {
		t.Fatalf("preflight: %d %v", status, hdr)
	}
	if _, resp, err := e.dial(http.Header{"Origin": {"https://evil.example"}}); err == nil || resp.StatusCode != http.StatusForbidden {
		t.Fatalf("a socket of another origin: %v", err)
	}
	c := watch(t, e.mustDial(http.Header{"Origin": {"https://betula.app"}}))
	c.send(hello(p, a, 0))
	c.expect("welcome")
	// Without Origins, the site's own origin: the one of the request's host.
	e2 := newEnv(t, nil)
	host := strings.TrimPrefix(e2.ts.URL, "http://")
	if _, resp, err := e2.dial(http.Header{"Origin": {"https://" + host}}); err != nil {
		t.Fatalf("the own origin: %v %v", err, resp)
	}
	if _, resp, err := e2.dial(http.Header{"Origin": {"https://betula.app"}}); err == nil || resp.StatusCode != http.StatusForbidden {
		t.Fatalf("another host: %v", err)
	}
	if _, err := New(Options{Store: e.st, Origins: []string{"betula.app"}}); err == nil {
		t.Error("an origin without a scheme")
	}
}

func TestHealthStatusAndTheRest(t *testing.T) {
	e := newEnv(t, nil)
	if status, _, _ := e.request(http.MethodGet, "/healthz", nil, nil); status != http.StatusOK {
		t.Errorf("healthz %d", status)
	}
	status, doc, _ := e.request(http.MethodGet, "/status", nil, nil)
	if status != http.StatusOK || doc["version"] != "test" || doc["spaces"] != float64(0) || doc["budget"] != float64(DefaultBudget) {
		t.Errorf("status %d %v", status, doc)
	}
	if status, _, _ := e.request(http.MethodGet, "/status", map[string]string{"X-Real-Ip": "203.0.113.1"}, nil); status != http.StatusNotFound {
		t.Errorf("status through a proxy: %d", status)
	}
	if status, doc, hdr := e.request(http.MethodPut, "/sync/v1/space", nil, nil); status != http.StatusMethodNotAllowed || doc["error"] != "method" || hdr.Get("Allow") == "" {
		t.Errorf("PUT: %d %v", status, doc)
	}
	if status, doc, _ := e.request(http.MethodGet, "/sync/v2/space", nil, nil); status != http.StatusNotFound || doc["error"] != "not_found" {
		t.Errorf("unknown path: %d %v", status, doc)
	}
	_, _, hdr := e.request(http.MethodGet, "/sync/v1/pair", nil, nil)
	if hdr.Get("Cache-Control") != "no-store" || hdr.Get("X-Content-Type-Options") != "nosniff" {
		t.Errorf("headers %v", hdr)
	}
}

func TestTheStatsFile(t *testing.T) {
	dir := t.TempDir()
	e := newEnv(t, func(o *Options) {
		o.StatsDir, o.Stack, o.StatsInterval, o.Build = dir, "phloem-test", 20*time.Millisecond, "0123456789abcdef"
	})
	p, a := newProfile(), randBytes(16)
	e.mustCreate(p, a)
	c, _ := e.connect(p, a, 0)
	c.push("1", "data")
	c.expect("stored")
	path := filepath.Join(dir, StatsFile)
	read := func() string {
		data, _ := os.ReadFile(path)
		return string(data)
	}
	eventually(t, "the stats file counts the entry", func() bool {
		return strings.Contains(read(), `phloem_entries_total{stack="phloem-test"} 1`)
	})
	text := read()
	for _, want := range []string{
		`phloem_up{stack="phloem-test"} 1`,
		`phloem_build_info{stack="phloem-test",version="test",build="0123456789ab"} 1`,
		`phloem_spaces{stack="phloem-test"} 1`,
		`phloem_spaces_created_total{stack="phloem-test"} 1`,
		`phloem_sockets{stack="phloem-test"} 1`,
		`phloem_devices{stack="phloem-test"} 1`,
		`phloem_budget_bytes{stack="phloem-test"} 5e+10`,
		"# TYPE phloem_entries_total counter",
	} {
		if !strings.Contains(text, want) {
			t.Errorf("the stats file lacks %q:\n%s", want, text)
		}
	}
	_ = e.srv.Close()
	if text := read(); !strings.Contains(text, `phloem_up{stack="phloem-test"} 0`) {
		t.Errorf("the last stats file:\n%s", text)
	}
	entries, _ := os.ReadDir(dir)
	if len(entries) != 1 {
		t.Errorf("the stats directory holds %v", entries)
	}
}

func TestWithLabel(t *testing.T) {
	in := "# HELP a_total A.\n# TYPE a_total counter\na_total 1\nb{x=\"1\"} 2\nc{} 3\n"
	want := "# HELP a_total A.\n# TYPE a_total counter\na_total{stack=\"s\"} 1\nb{stack=\"s\",x=\"1\"} 2\nc{stack=\"s\"} 3\n"
	if got := string(withLabel([]byte(in), "stack", "s")); got != want {
		t.Errorf("withLabel:\n%s\nwant\n%s", got, want)
	}
	if got := string(withLabel([]byte(in), "stack", "")); got != in {
		t.Errorf("no label: %s", got)
	}
}

func TestStatusIsJSON(t *testing.T) {
	e := newEnv(t, nil)
	data, err := json.Marshal(e.srv.Status())
	if err != nil || !strings.Contains(string(data), `"budget_used":0`) {
		t.Errorf("%s %v", data, err)
	}
}
