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
	if status, doc := take(b64s(one)); status != http.StatusNotFound || doc["error"] != "taken" {
		t.Fatalf("a second take: %d %v", status, doc)
	}
	if status, doc := take(b64s(randBytes(16))); status != http.StatusNotFound || doc["error"] != "not_found" {
		t.Fatalf("an id that never was: %d %v", status, doc)
	}
	two := randBytes(16)
	leave(p.headers(), two, []byte("x"))
	e.clock.Advance(10*time.Minute + time.Second)
	if status, doc := take(b64s(two)); status != http.StatusNotFound || doc["error"] != "expired" {
		t.Fatalf("an expired parcel: %d %v", status, doc)
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

func TestParcelsWaitAsLongAsTheyAsk(t *testing.T) {
	e := newEnv(t, nil)
	p := newProfile()
	e.mustCreate(p, randBytes(16))
	for ttl, want := range map[int64]int{59: 400, 604801: 400, -1: 400, 1 << 62: 400, 60: 201, 604800: 201} {
		if status, doc := e.leavePair(p, randBytes(16), "x", ttl); status != want {
			t.Errorf("ttl %d: %d %v, want %d", ttl, status, doc, want)
		}
	}
	for _, ttl := range []string{"0", "60.5", `"600"`, "true"} {
		body := `{"id":"` + b64s(randBytes(16)) + `","box":"eA","ttl":` + ttl + `}`
		if status, doc, _ := e.request(http.MethodPost, "/sync/v1/pair", p.headers(), body); status != http.StatusBadRequest || doc["error"] != "invalid" {
			t.Errorf("ttl %s: %d %v", ttl, status, doc)
		}
	}
	week, plain, named := randBytes(16), randBytes(16), randBytes(16)
	if status, _ := e.leavePair(p, week, "a week", 604800); status != http.StatusCreated {
		t.Fatal(status)
	}
	if status, _ := e.leavePair(p, plain, "ten minutes", 0); status != http.StatusCreated {
		t.Fatal(status)
	}
	body := `{"id":"` + b64s(named) + `","box":"eA","ttl":null}`
	if status, doc, _ := e.request(http.MethodPost, "/sync/v1/pair", p.headers(), body); status != http.StatusCreated {
		t.Fatalf("ttl null: %d %v", status, doc)
	}
	status, doc, _ := e.request(http.MethodGet, "/sync/v1/pairs", p.headers(), nil)
	if status != http.StatusOK {
		t.Fatalf("pairs: %d %v", status, doc)
	}
	at := map[string]float64{}
	for _, raw := range doc["parcels"].([]any) {
		parcel := raw.(map[string]any)
		if parcel["state"] != "waiting" {
			t.Errorf("parcel %v", parcel)
		}
		at[parcel["id"].(string)] = parcel["at"].(float64)
	}
	now := float64(e.clock.Now().UnixMilli())
	if at[b64s(week)] != now+604800e3 || at[b64s(plain)] != now+600e3 || at[b64s(named)] != now+600e3 || len(at) != 5 {
		t.Errorf("when they expire: %v (now %v)", at, now)
	}
	e.clock.Advance(6 * 24 * time.Hour)
	if status, doc := e.takePair(week); status != http.StatusOK || decoded(t, doc["box"].(string)) != "a week" {
		t.Errorf("a parcel of a week after six days: %d %v", status, doc)
	}
	if status, doc := e.takePair(plain); status != http.StatusNotFound || doc["error"] != "expired" {
		t.Errorf("a parcel of ten minutes after six days: %d %v", status, doc)
	}
}

// What became of a parcel: taken, expired, withdrawn, for 30 days; then nothing.
func TestParcelsSayWhatBecameOfThem(t *testing.T) {
	e := newEnv(t, nil)
	p, q := newProfile(), newProfile()
	e.mustCreate(p, randBytes(16))
	e.mustCreate(q, randBytes(16))
	taken, expired, withdrawn, waiting := randBytes(16), randBytes(16), randBytes(16), randBytes(16)
	for id, ttl := range map[*[]byte]int64{&taken: 0, &expired: 0, &withdrawn: 0, &waiting: 3600} {
		if status, doc := e.leavePair(p, *id, "box", ttl); status != http.StatusCreated {
			t.Fatalf("leave: %d %v", status, doc)
		}
	}
	start := e.clock.Now()
	withdraw := func(who map[string]string, id string) int {
		t.Helper()
		hdr := map[string]string{"Betula-Pair": id}
		for k, v := range who {
			hdr[k] = v
		}
		status, _, _ := e.request(http.MethodDelete, "/sync/v1/pair", hdr, nil)
		return status
	}
	e.clock.Advance(time.Minute)
	if status, _ := e.takePair(taken); status != http.StatusOK {
		t.Fatal(status)
	}
	e.clock.Advance(time.Minute)
	if status := withdraw(p.headers(), b64s(withdrawn)); status != http.StatusNoContent {
		t.Fatalf("withdraw: %d", status)
	}
	// 204 for what is not the space's, waits no longer, or never was: nothing changes.
	for name, c := range map[string]struct {
		who  map[string]string
		id   []byte
		want int
	}{
		"another space's": {q.headers(), waiting, 204},
		"taken":           {p.headers(), taken, 204},
		"withdrawn":       {p.headers(), withdrawn, 204},
		"never was":       {p.headers(), randBytes(16), 204},
		"no token":        {map[string]string{"Betula-Space": b64s(p.space)}, waiting, 401},
		"a short id":      {p.headers(), randBytes(15), 400},
	} {
		if status := withdraw(c.who, b64s(c.id)); status != c.want {
			t.Errorf("withdraw, %s: %d, want %d", name, status, c.want)
		}
	}
	e.clock.Advance(9 * time.Minute) // expired, and the sweep has not run
	for id, want := range map[*[]byte]string{&taken: "taken", &expired: "expired", &withdrawn: "withdrawn"} {
		if status, doc := e.takePair(*id); status != http.StatusNotFound || doc["error"] != want {
			t.Errorf("%s: %d %v", want, status, doc)
		}
	}
	status, doc, _ := e.request(http.MethodGet, "/sync/v1/pairs", p.headers(), nil)
	if status != http.StatusOK {
		t.Fatalf("pairs: %d %v", status, doc)
	}
	var got []string
	for _, raw := range doc["parcels"].([]any) {
		parcel := raw.(map[string]any)
		name := map[string]string{b64s(taken): "taken", b64s(expired): "expired", b64s(withdrawn): "withdrawn", b64s(waiting): "waiting"}[parcel["id"].(string)]
		got = append(got, fmt.Sprintf("%s:%s@%v", name, parcel["state"], time.UnixMilli(int64(parcel["at"].(float64))).Sub(start)))
	}
	if want := "[waiting:waiting@1h0m0s expired:expired@10m0s withdrawn:withdrawn@2m0s taken:taken@1m0s]"; fmt.Sprint(got) != want {
		t.Errorf("parcels %v, want %s", got, want)
	}
	if status, doc, _ := e.request(http.MethodGet, "/sync/v1/pairs", q.headers(), nil); status != http.StatusOK || fmt.Sprint(doc["parcels"]) != "[]" {
		t.Errorf("another space's parcels: %d %v", status, doc)
	}
	if status, _, _ := e.request(http.MethodGet, "/sync/v1/pairs", profile{p.space, randBytes(32)}.headers(), nil); status != http.StatusUnauthorized {
		t.Errorf("pairs with a wrong token: %d", status)
	}
	// 30 days on, what became of them is forgotten; the one that waited an hour expired since.
	e.clock.Advance(30 * 24 * time.Hour)
	e.srv.sweep()
	for _, id := range [][]byte{taken, expired, withdrawn} {
		if status, doc := e.takePair(id); status != http.StatusNotFound || doc["error"] != "not_found" {
			t.Errorf("after 30 days: %d %v", status, doc)
		}
	}
	_, doc, _ = e.request(http.MethodGet, "/sync/v1/pairs", p.headers(), nil)
	if list := doc["parcels"].([]any); len(list) != 1 || list[0].(map[string]any)["state"] != "expired" {
		t.Errorf("parcels after 30 days: %v", list)
	}
	var buf strings.Builder
	_ = e.srv.reg.WriteText(&buf)
	for _, want := range []string{
		`phloem_parcels_total{event="left"} 4`,
		`phloem_parcels_total{event="taken"} 1`,
		`phloem_parcels_total{event="withdrawn"} 1`,
		`phloem_parcels_total{event="expired"} 2`,
	} {
		if !strings.Contains(buf.String(), want) {
			t.Errorf("metrics lack %s:\n%s", want, buf.String())
		}
	}
}

func TestThirtyTwoParcelsWait(t *testing.T) {
	e := newEnv(t, nil)
	p := newProfile()
	e.mustCreate(p, randBytes(16))
	first := randBytes(16)
	if status, _ := e.leavePair(p, first, "x", 0); status != http.StatusCreated {
		t.Fatal(status)
	}
	for range 31 {
		if status, doc := e.leavePair(p, randBytes(16), "x", 604800); status != http.StatusCreated {
			t.Fatalf("leave: %d %v", status, doc)
		}
	}
	if status, doc := e.leavePair(p, randBytes(16), "x", 0); status != http.StatusTooManyRequests || doc["error"] != "slow_down" {
		t.Fatalf("a 33rd parcel: %d %v", status, doc)
	}
	// One withdrawn makes room; the id of what was is not taken again.
	hdr := p.headers()
	hdr["Betula-Pair"] = b64s(first)
	if status, _, _ := e.request(http.MethodDelete, "/sync/v1/pair", hdr, nil); status != http.StatusNoContent {
		t.Fatal(status)
	}
	if status, doc := e.leavePair(p, first, "x", 0); status != http.StatusConflict || doc["error"] != "exists" {
		t.Fatalf("a parcel of a withdrawn one's id: %d %v", status, doc)
	}
	if status, doc := e.leavePair(p, randBytes(16), "x", 0); status != http.StatusCreated {
		t.Fatalf("a parcel after one was withdrawn: %d %v", status, doc)
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
	// The parts: PUT, their header and If-None-Match asked for, their ETag read.
	status, _, hdr = e.request(http.MethodOptions, "/sync/v1/part", map[string]string{
		"Origin": "https://betula.app", "Access-Control-Request-Method": "PUT", "Access-Control-Request-Headers": "betula-part, if-none-match"}, nil)
	if allow := hdr.Get("Access-Control-Allow-Headers"); status != http.StatusNoContent || !strings.Contains(hdr.Get("Access-Control-Allow-Methods"), "PUT") ||
		!strings.Contains(allow, "Betula-Part") || !strings.Contains(allow, "If-None-Match") {
		t.Fatalf("preflight of a part: %d %v", status, hdr)
	}
	part := randBytes(16)
	e.mustPutPart(p, part, "x", 1)
	status, _, hdr = e.request(http.MethodGet, "/sync/v1/part", map[string]string{"Origin": "https://betula.app", "Betula-Part": b64s(part)}, nil)
	if status != http.StatusOK || hdr.Get("Access-Control-Expose-Headers") != "ETag" || hdr.Get("ETag") != `"1"` {
		t.Fatalf("a part across origins: %d %v", status, hdr)
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
	for path, allow := range map[string]string{
		"/sync/v1/pair":  "GET, POST, DELETE, OPTIONS",
		"/sync/v1/pairs": "GET, OPTIONS",
		"/sync/v1/part":  "GET, PUT, DELETE, OPTIONS",
	} {
		if status, doc, hdr := e.request(http.MethodPatch, path, nil, nil); status != http.StatusMethodNotAllowed || doc["error"] != "method" || hdr.Get("Allow") != allow {
			t.Errorf("PATCH %s: %d %v %q", path, status, doc, hdr.Get("Allow"))
		}
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
