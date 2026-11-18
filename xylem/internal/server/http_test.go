package server

import (
	"encoding/json"
	"net/http"
	"net/netip"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/store"
)

// strict is a Xylem as serve starts it: no endpoint but a push service's, over https.
func strict(o *Options) {
	o.Insecure = false
	o.Hosts = nil
}

func TestKeyIsThePublicHalf(t *testing.T) {
	w := newWorld(t)
	rec := w.do(http.MethodGet, "/push/v1/key", "203.0.113.9", nil, "Origin", "https://betula.test")
	var out struct{ Key string }
	if rec.Code != http.StatusOK || json.Unmarshal(rec.Body.Bytes(), &out) != nil || out.Key != w.srv.key.Public() {
		t.Fatalf("%d %s", rec.Code, rec.Body)
	}
	if strings.Contains(rec.Body.String(), w.srv.key.String()) {
		t.Fatal("the private key is in the answer")
	}
	if got := rec.Header().Get("Access-Control-Allow-Origin"); got != "https://betula.test" {
		t.Fatalf("Access-Control-Allow-Origin %q", got)
	}
}

func TestOnlyPushServicesAreEndpoints(t *testing.T) {
	w := newWorld(t, strict)
	b := w.push.browser()
	put := func(endpoint string) int {
		return w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", sub(endpoint, b.private, b.auth, "de", "exams")).Code
	}
	for _, endpoint := range []string{
		"https://fcm.googleapis.com/fcm/send/dQw4:APA91b",
		"https://fcm.googleapis.com/wp/dQw4",
		"https://jmt17.google.com/fcm/send/dQw4",
		"https://updates.push.services.mozilla.com/wpush/v2/gAAAAAB",
		"https://web.push.apple.com/QGuy8",
		"https://wns2-by3p.notify.windows.com/w/?token=BQYAAAD%2f",
		"https://FCM.googleapis.com:443/fcm/send/upper",
	} {
		if code := put(endpoint); code != http.StatusNoContent {
			t.Errorf("%s: %d, want 204", endpoint, code)
		}
	}
	if n := w.st.Subscriptions(); n != 7 {
		t.Fatalf("%d subscriptions, want 7", n)
	}
	for _, endpoint := range []string{
		"",
		"http://fcm.googleapis.com/fcm/send/x", // not https
		"https://fcm.googleapis.com:8443/fcm/send/x",        // another port
		"https://user:pw@fcm.googleapis.com/fcm/send/x",     // a user
		"https://fcm.googleapis.com.evil.example/fcm/send/", // the name as a prefix
		"https://xfcm.googleapis.com/fcm/send/x",            // ... as a suffix of another name
		"https://evil.example/?https://fcm.googleapis.com/", // ... in the query
		"https://evil.example/fcm.googleapis.com",           //
		"https://storage.googleapis.com/bucket/x",           // Google's, and no push service
		"https://push.apple.com.evil.example/x",             //
		"https://127.0.0.1/x",                               // the house
		"https://169.254.169.254/latest/meta-data/",         //
		"https://[::1]/x",                                   //
		"https://localhost/x",                               //
		"//fcm.googleapis.com/x",                            // no scheme
		"fcm.googleapis.com/x",                              //
		"https://fcm.googleapis.com/a b",                    // a space
		"https://fcm.googleapis.com/" + strings.Repeat("a", 2100),
		b.endpoint(), // the test's own push service, which serve would never take
	} {
		if code := put(endpoint); code != http.StatusBadRequest {
			t.Errorf("%q: %d, want 400", endpoint, code)
		}
	}
	if n := w.st.Subscriptions(); n != 7 {
		t.Fatalf("%d subscriptions after the refusals, want 7", n)
	}
	// The hosts that were refused are what `xylem status` shows: how one learns of a push
	// service Xylem does not know. Nothing but the host.
	rec := w.do(http.MethodGet, "/status", "", nil)
	var st Status
	if err := json.Unmarshal(rec.Body.Bytes(), &st); err != nil {
		t.Fatal(err)
	}
	if st.Refused["evil.example"] != 2 || st.Refused["storage.googleapis.com"] != 1 || st.Refused["fcm.googleapis.com"] == 0 {
		t.Fatalf("refused %v", st.Refused)
	}
	if strings.Contains(rec.Body.String(), "bucket") || strings.Contains(rec.Body.String(), "meta-data") {
		t.Fatalf("a path of a refused endpoint is in the status: %s", rec.Body)
	}
}

// The audience of a token is the push service's origin as the service itself names it.
func TestOriginOfAnEndpoint(t *testing.T) {
	w := newWorld(t, strict)
	for endpoint, want := range map[string]string{
		"https://fcm.googleapis.com/fcm/send/x":      "https://fcm.googleapis.com",
		"https://FCM.googleapis.com:443/fcm/send/x":  "https://fcm.googleapis.com",
		"https://web.push.apple.com/QGuy8":           "https://web.push.apple.com",
		"https://db5p.notify.windows.com/w/?token=x": "https://db5p.notify.windows.com",
	} {
		ep, err := w.srv.endpointOf(endpoint)
		if err != nil || ep.origin != want {
			t.Errorf("%s: origin %q, %v; want %q", endpoint, ep.origin, err, want)
		}
	}
	loose := newWorld(t)
	if ep, err := loose.srv.endpointOf("http://127.0.0.1:8123/wpush/v2/x"); err != nil || ep.origin != "http://127.0.0.1:8123" || ep.service != "other" {
		t.Errorf("the test's own: %+v, %v", ep, err)
	}
}

func TestMoreHostsCanBeTaken(t *testing.T) {
	w := newWorld(t, func(o *Options) {
		o.Insecure = false
		o.Hosts = []string{"push.example.org", " .push.example.net "}
	})
	b := w.push.browser()
	for endpoint, want := range map[string]int{
		"https://push.example.org/s/1":      http.StatusNoContent,
		"https://eu.push.example.net/s/1":   http.StatusNoContent,
		"https://a.push.example.org/s/1":    http.StatusBadRequest, // named exactly
		"https://push.example.net/s/1":      http.StatusBadRequest, // every host under it, not itself
		"https://fcm.googleapis.com/wp/abc": http.StatusNoContent,  // the ones it knows stay
	} {
		if code := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", sub(endpoint, b.private, b.auth, "de", "exams")).Code; code != want {
			t.Errorf("%s: %d, want %d", endpoint, code, want)
		}
	}
	for _, hosts := range [][]string{{"https://push.example.org"}, {"push_example"}, {"nodots"}, {"a..b"}} {
		st, _ := store.Open(t.TempDir())
		if _, err := New(Options{Store: st, Key: w.srv.key, Site: "https://betula.test", Hosts: hosts}); err == nil {
			t.Errorf("hosts %v were taken", hosts)
		}
		_ = st.Close()
	}
}

func TestSubscriptionIsChecked(t *testing.T) {
	w := newWorld(t)
	b := w.push.browser()
	good := func() map[string]any { return sub(b.endpoint(), b.private, b.auth, "de", "exams") }
	with := func(change func(m map[string]any)) map[string]any {
		m := good()
		change(m)
		return m
	}
	keys := func(p256dh, auth string) func(m map[string]any) {
		return func(m map[string]any) { m["keys"] = map[string]string{"p256dh": p256dh, "auth": auth} }
	}
	ok := good()["keys"].(map[string]string)
	many := make([]string, 17)
	for i := range many {
		many[i] = "topic-" + string(rune('a'+i))
	}
	for name, c := range map[string]struct {
		body any
		code int
		err  string
	}{
		"no JSON":           {`{"endpoint":`, 400, "invalid"},
		"an array":          {`[]`, 400, "invalid"},
		"too large":         {`{"endpoint":"` + strings.Repeat("a", maxBody) + `"}`, 413, "too_big"},
		"no keys":           {with(func(m map[string]any) { delete(m, "keys") }), 400, "keys"},
		"a short p256dh":    {with(keys(ok["p256dh"][:80], ok["auth"])), 400, "keys"},
		"p256dh off P-256":  {with(keys("BAEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE", ok["auth"])), 400, "keys"},
		"a short auth":      {with(keys(ok["p256dh"], "AAAA")), 400, "keys"},
		"keys in base64":    {with(keys(strings.NewReplacer("-", "+", "_", "/").Replace(ok["p256dh"])+"+", ok["auth"])), 400, "invalid"},
		"a language of ten": {with(func(m map[string]any) { m["lang"] = "germanlang" }), 400, "lang"},
		"a topic in caps":   {with(func(m map[string]any) { m["topics"] = []string{"Exams"} }), 400, "topics"},
		"a topic of spaces": {with(func(m map[string]any) { m["topics"] = []string{"exam period"} }), 400, "topics"},
		"a topic too long":  {with(func(m map[string]any) { m["topics"] = []string{strings.Repeat("a", 33)} }), 400, "topics"},
		"seventeen topics":  {with(func(m map[string]any) { m["topics"] = many }), 400, "topics"},
		"padded keys":       {with(keys(ok["p256dh"]+"=", ok["auth"]+"==")), 204, ""},
		"sixteen topics":    {with(func(m map[string]any) { m["topics"] = many[:16] }), 204, ""},
		"a topic twice":     {with(func(m map[string]any) { m["topics"] = []string{"exams", "exams", "a-1"} }), 204, ""},
		"a region":          {with(func(m map[string]any) { m["lang"] = "pt-BR" }), 204, ""},
	} {
		rec := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", c.body)
		if rec.Code != c.code || errorOf(rec) != c.err {
			t.Errorf("%s: %d %q, want %d %q", name, rec.Code, errorOf(rec), c.code, c.err)
		}
	}
	for path, methods := range map[string][]string{
		"/push/v1/subscription": {http.MethodGet, http.MethodPost},
		"/push/v1/key":          {http.MethodPut, http.MethodDelete},
	} {
		for _, method := range methods {
			if rec := w.do(method, path, "203.0.113.9", nil); rec.Code != http.StatusMethodNotAllowed || rec.Header().Get("Allow") == "" {
				t.Errorf("%s %s: %d", method, path, rec.Code)
			}
		}
	}
	if rec := w.do(http.MethodGet, "/push/v2/key", "203.0.113.9", nil); rec.Code != http.StatusNotFound {
		t.Errorf("another version: %d", rec.Code)
	}
}

// Put again, a subscription is replaced; without topics, or deleted, it is forgotten.
func TestSubscriptionIsReplacedAndForgotten(t *testing.T) {
	w := newWorld(t)
	b := w.subscribe("de", "exams", "semester")
	topics := func() string {
		got, lang, found, err := w.st.Topics(t.Context(), b.endpoint())
		if err != nil {
			t.Fatal(err)
		}
		if !found {
			return "none"
		}
		return lang + " " + strings.Join(got, ",")
	}
	if got := topics(); got != "de exams,semester" {
		t.Fatal(got)
	}
	w.put(b, "en", "breaks")
	if got := topics(); got != "en breaks" || w.st.Subscriptions() != 1 {
		t.Fatalf("%s, %d subscriptions", got, w.st.Subscriptions())
	}
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")})
	if len(b.messages()) != 0 {
		t.Fatal("told of a topic it no longer listens to")
	}
	w.put(b, "en")
	if got := topics(); got != "none" || w.st.Subscriptions() != 0 {
		t.Fatalf("after no topics: %s, %d subscriptions", got, w.st.Subscriptions())
	}

	w.put(b, "de", "exams")
	del := func(body any) int { return w.do(http.MethodDelete, "/push/v1/subscription", "203.0.113.9", body).Code }
	if code := del(map[string]string{"endpoint": b.endpoint()}); code != http.StatusNoContent || w.st.Subscriptions() != 0 {
		t.Fatalf("delete: %d, %d subscriptions", code, w.st.Subscriptions())
	}
	// Again, and for an endpoint Xylem never had: the same answer.
	if del(map[string]string{"endpoint": b.endpoint()}) != http.StatusNoContent || del(map[string]string{"endpoint": "https://fcm.googleapis.com/never"}) != http.StatusNoContent {
		t.Fatal("deleting what is not there did not answer 204")
	}
	if del(map[string]string{}) != http.StatusBadRequest || del(`{`) != http.StatusBadRequest {
		t.Fatal("a delete without an endpoint was taken")
	}
}

func TestOnlyTheSitesOriginMayAsk(t *testing.T) {
	w := newWorld(t)
	b := w.push.browser()
	body := sub(b.endpoint(), b.private, b.auth, "de", "exams")
	// Without Origins: the origin of the request's own host.
	if rec := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", body, "Origin", "https://betula.test"); rec.Code != http.StatusNoContent {
		t.Fatalf("the site's own origin: %d", rec.Code)
	}
	for _, origin := range []string{"https://evil.example", "null", "https://betula.test.evil.example"} {
		rec := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", body, "Origin", origin)
		if rec.Code != http.StatusForbidden || rec.Header().Get("Access-Control-Allow-Origin") != "" {
			t.Errorf("origin %q: %d", origin, rec.Code)
		}
	}
	// The preflight of a PUT with JSON.
	rec := w.do(http.MethodOptions, "/push/v1/subscription", "203.0.113.9", nil, "Origin", "https://betula.test",
		"Access-Control-Request-Method", "PUT", "Access-Control-Request-Headers", "content-type")
	if rec.Code != http.StatusNoContent || !strings.Contains(rec.Header().Get("Access-Control-Allow-Methods"), "PUT") ||
		rec.Header().Get("Access-Control-Allow-Headers") != "Content-Type" || rec.Header().Get("Vary") != "Origin" {
		t.Fatalf("preflight: %d %v", rec.Code, rec.Header())
	}

	// With Origins (the dev server next to Xylem): exactly those.
	dev := newWorld(t, func(o *Options) { o.Origins = []string{"http://127.0.0.1:8080"} })
	if rec := dev.do(http.MethodGet, "/push/v1/key", "203.0.113.9", nil, "Origin", "http://127.0.0.1:8080"); rec.Code != http.StatusOK {
		t.Fatalf("a listed origin: %d", rec.Code)
	}
	if rec := dev.do(http.MethodGet, "/push/v1/key", "203.0.113.9", nil, "Origin", "https://betula.test"); rec.Code != http.StatusForbidden {
		t.Fatalf("the host's own origin, not listed: %d", rec.Code)
	}
}

// The commands' side is there for a command of this machine alone: not through Traefik, not
// for a page open in a browser of this machine.
func TestCommandsSideIsLocal(t *testing.T) {
	w := newWorld(t)
	req := AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")}
	for name, rec := range map[string]int{
		"through Traefik": w.do(http.MethodPost, "/announcements", "203.0.113.9", req).Code,
		"status, Traefik": w.do(http.MethodGet, "/status", "203.0.113.9", nil).Code,
		"list, Traefik":   w.do(http.MethodGet, "/announcements", "203.0.113.9", nil).Code,
		"cancel, Traefik": w.do(http.MethodDelete, "/announcements/1", "203.0.113.9", nil).Code,
		"a page here":     w.do(http.MethodPost, "/announcements", "", req, "Origin", "http://127.0.0.1:8300").Code,
		"a page anywhere": w.do(http.MethodPost, "/announcements", "", req, "Origin", "https://evil.example").Code,
		"forwarded":       w.do(http.MethodGet, "/status", "", nil, "X-Forwarded-For", "127.0.0.1").Code,
	} {
		if rec != http.StatusNotFound {
			t.Errorf("%s: %d, want 404", name, rec)
		}
	}
	if list, err := w.st.Announcements(t.Context(), 10); err != nil || len(list) != 0 {
		t.Fatalf("an announcement was made from outside: %v %v", list, err)
	}
	if rec := w.do(http.MethodGet, "/status", "", nil); rec.Code != http.StatusOK {
		t.Fatalf("a command of this machine: %d", rec.Code)
	}
	if rec := w.do(http.MethodPut, "/announcements", "", req); rec.Code != http.StatusMethodNotAllowed {
		t.Fatalf("PUT /announcements: %d", rec.Code)
	}
	if rec := w.do(http.MethodGet, "/healthz", "203.0.113.9", nil); rec.Code != http.StatusOK || rec.Body.String() != "ok\n" {
		t.Fatalf("healthz: %d %q", rec.Code, rec.Body)
	}
}

func TestLimitsPerAddress(t *testing.T) {
	w := newWorld(t, func(o *Options) {
		o.Limits.NewPerAddr, o.Limits.NewTotal, o.Limits.WritesPerAddr = 2, 5, 8
		o.MaxSubscriptions = 4
	})
	put := func(b *browser, addr string) int {
		return w.do(http.MethodPut, "/push/v1/subscription", addr, sub(b.endpoint(), b.private, b.auth, "de", "exams")).Code
	}
	first, second, third := w.push.browser(), w.push.browser(), w.push.browser()
	if put(first, "198.51.100.1") != 204 || put(second, "198.51.100.1") != 204 {
		t.Fatal("the first two of an address")
	}
	rec := w.do(http.MethodPut, "/push/v1/subscription", "198.51.100.1", sub(third.endpoint(), third.private, third.auth, "de", "exams"))
	if rec.Code != http.StatusTooManyRequests || rec.Header().Get("Retry-After") == "" || errorOf(rec) != "slow_down" {
		t.Fatalf("the third new one of an address: %d %s", rec.Code, rec.Body)
	}
	// One it holds already is brought up to date all the same, and costs nothing new.
	if put(first, "198.51.100.1") != 204 || put(first, "198.51.100.1") != 204 {
		t.Fatal("an update was refused as a new subscription")
	}
	// Another address, and an hour later the same one.
	if put(third, "198.51.100.2") != 204 {
		t.Fatal("another address")
	}
	w.clock.Add(time.Hour)
	fourth, fifth := w.push.browser(), w.push.browser()
	if put(fourth, "198.51.100.1") != 204 {
		t.Fatal("an hour later")
	}
	// Four are held: no fifth, until one leaves.
	rec = w.do(http.MethodPut, "/push/v1/subscription", "198.51.100.3", sub(fifth.endpoint(), fifth.private, fifth.auth, "de", "exams"))
	if rec.Code != http.StatusInsufficientStorage || errorOf(rec) != "full" {
		t.Fatalf("a fifth: %d %s", rec.Code, rec.Body)
	}
	if put(first, "198.51.100.3") != 204 {
		t.Fatal("an update while full")
	}
	if code := w.do(http.MethodDelete, "/push/v1/subscription", "198.51.100.3", map[string]string{"endpoint": first.endpoint()}).Code; code != 204 {
		t.Fatal("delete", code)
	}
	if put(fifth, "198.51.100.3") != 204 {
		t.Fatal("a fifth after one left")
	}
	// Eight writes an hour of one address, whatever they are.
	for i := range 8 {
		code := put(fifth, "2001:db8:1:2::"+string(rune('1'+i)))
		if want := http.StatusNoContent; code != want {
			t.Fatalf("write %d of a /64: %d", i+1, code)
		}
	}
	if code := put(fifth, "2001:db8:1:2:ffff::9"); code != http.StatusTooManyRequests {
		t.Fatalf("the ninth write of a /64: %d", code)
	}
	if code := put(fifth, "2001:db8:1:3::1"); code != http.StatusNoContent {
		t.Fatalf("the /64 next to it: %d", code)
	}
}

func TestAnnouncementIsChecked(t *testing.T) {
	w := newWorld(t)
	ok := func() AnnounceRequest {
		return AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "Ab heute.")}
	}
	with := func(change func(r *AnnounceRequest)) AnnounceRequest {
		r := ok()
		change(&r)
		return r
	}
	long := strings.Repeat("ä", 121)
	for name, c := range map[string]struct {
		req  AnnounceRequest
		want string
	}{
		"no topic":            {with(func(r *AnnounceRequest) { r.Topic = "" }), "topic"},
		"a topic in caps":     {with(func(r *AnnounceRequest) { r.Topic = "Exams" }), "topic"},
		"no texts":            {with(func(r *AnnounceRequest) { r.Texts = nil }), "texts: none"},
		"no default language": {with(func(r *AnnounceRequest) { r.Texts = textsOf("en", "Exams", "") }), `none in "de"`},
		"no title":            {with(func(r *AnnounceRequest) { r.Texts = textsOf("de", "  ", "Text") }), "no title"},
		"a long title":        {with(func(r *AnnounceRequest) { r.Texts = textsOf("de", long, "") }), "121 characters, 120 at most"},
		"a long body":         {with(func(r *AnnounceRequest) { r.Texts = textsOf("de", "Titel", strings.Repeat("x", 601)) }), "601 characters"},
		"no language":         {with(func(r *AnnounceRequest) { r.Texts["Deutsch"] = store.Text{Title: "x"} }), "is no language"},
		"another site":        {with(func(r *AnnounceRequest) { r.Link = "https://evil.example/" }), "a path of the site"},
		"another site, short": {with(func(r *AnnounceRequest) { r.Link = "//evil.example/" }), "a path of the site"},
		"a backslash":         {with(func(r *AnnounceRequest) { r.Link = `/\evil.example` }), "a path of the site"},
		"a language's link":   {with(func(r *AnnounceRequest) { r.Texts["de"] = store.Text{Title: "x", Link: "javascript:alert(1)"} }), `the link in "de"`},
		"an urgency":          {with(func(r *AnnounceRequest) { r.Urgency = "urgent" }), "urgency"},
		"a key with a space":  {with(func(r *AnnounceRequest) { r.Key = "exams 2026" }), "key"},
		"no duration":         {with(func(r *AnnounceRequest) { r.For = "a day" }), "a duration"},
		"too short":           {with(func(r *AnnounceRequest) { r.For = "30s" }), "between"},
		"too long":            {with(func(r *AnnounceRequest) { r.For = "700h" }), "between"},
		"nine languages": {with(func(r *AnnounceRequest) {
			r.Texts = textsOf("de", "a", "", "en", "a", "", "fr", "a", "", "pl", "a", "", "cs", "a", "", "es", "a", "", "it", "a", "", "uk", "a", "", "tr", "a", "")
		}), "9 languages"},
	} {
		rec := w.do(http.MethodPost, "/announcements", "", c.req)
		if rec.Code != http.StatusBadRequest || !strings.Contains(errorOf(rec), c.want) {
			t.Errorf("%s: %d %q, want 400 with %q", name, rec.Code, errorOf(rec), c.want)
		}
	}
	if list, _ := w.st.Announcements(t.Context(), 10); len(list) != 0 {
		t.Fatalf("%d announcements were kept", len(list))
	}

	// As long as it may be, in every language, with every urgency: taken, and it fits a push
	// message.
	full := with(func(r *AnnounceRequest) {
		r.Key, r.Urgency, r.For, r.Link = "exams.2026:w-1", "high", "672h", "/"+strings.Repeat("a", 511)
		r.Texts = map[string]store.Text{}
		for _, lang := range []string{"de", "en", "fr", "pl", "cs", "es", "it", "uk"} {
			r.Texts[lang] = store.Text{Title: strings.Repeat("ä", 120), Body: strings.Repeat("ö", 600), Link: "/" + lang + "/" + strings.Repeat("b", 500)}
		}
	})
	b := w.subscribe("uk", "exams")
	out := w.announce(full)
	if !out.Made || len(b.messages()) != 1 || b.messages()[0].Urgency != "high" {
		t.Fatalf("the largest announcement: %+v, %d messages", out, len(b.messages()))
	}

	// A dry run says whom it would reach and keeps nothing.
	rec := w.do(http.MethodPost, "/announcements", "", with(func(r *AnnounceRequest) { r.Dry = true }))
	var dry AnnounceResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &dry); err != nil || rec.Code != http.StatusOK || dry.Made || dry.Reach["uk"] != 1 || dry.Announcement.ID != 0 {
		t.Fatalf("dry run: %d %s", rec.Code, rec.Body)
	}
	if list, _ := w.st.Announcements(t.Context(), 10); len(list) != 1 {
		t.Fatalf("%d announcements after a dry run, want 1", len(list))
	}

	// Where the limits of the texts were raised, the push message's own size still holds.
	big := newWorld(t, func(o *Options) { o.Limits.MaxBody = 5000 })
	rec = big.do(http.MethodPost, "/announcements", "", AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Titel", strings.Repeat("ö", 2000))})
	if rec.Code != http.StatusBadRequest || !strings.Contains(errorOf(rec), "a push message carries 3993") {
		t.Fatalf("a notification above a push message: %d %s", rec.Code, rec.Body)
	}
}

func TestStatusAndList(t *testing.T) {
	w := newWorld(t, func(o *Options) { o.Stack = "xylem-canary" })
	w.subscribe("de", "exams", "semester")
	w.subscribe("en", "exams")
	at := start.Add(72 * time.Hour)
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Jetzt", "")})
	w.announce(AnnounceRequest{Topic: "breaks", At: &at, Texts: textsOf("de", "Später", "")})

	var st Status
	rec := w.do(http.MethodGet, "/status", "", nil)
	if err := json.Unmarshal(rec.Body.Bytes(), &st); err != nil {
		t.Fatal(err)
	}
	if st.Version != "test" || st.Stack != "xylem-canary" || st.Site != "https://betula.test" || st.Lang != "de" || st.Key != w.srv.key.Public() ||
		st.Subscriptions != 2 || st.Topics["exams"] != 2 || st.Topics["semester"] != 1 || st.Languages["de"] != 1 || st.Languages["en"] != 1 ||
		st.Announcements["waiting"] != 1 || st.Retries != 0 || st.Next == nil || !st.Next.Equal(at) {
		t.Fatalf("status %s", rec.Body)
	}
	if strings.Contains(rec.Body.String(), "/wpush/") || strings.Contains(rec.Body.String(), w.srv.key.String()) {
		t.Fatalf("an endpoint or the private key is in the status: %s", rec.Body)
	}

	var list struct {
		Lang          string
		Announcements []AnnouncementView
	}
	rec = w.do(http.MethodGet, "/announcements", "", nil)
	if err := json.Unmarshal(rec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if list.Lang != "de" || len(list.Announcements) != 2 || list.Announcements[0].State != store.Waiting || list.Announcements[0].Finished != nil ||
		list.Announcements[1].State != store.Done || list.Announcements[1].Sent != 2 || list.Announcements[1].Finished == nil {
		t.Fatalf("list %s", rec.Body)
	}
	rec = w.do(http.MethodGet, "/announcements?limit=1", "", nil)
	_ = json.Unmarshal(rec.Body.Bytes(), &list)
	if len(list.Announcements) != 1 || list.Announcements[0].Topic != "breaks" {
		t.Fatalf("limit=1: %s", rec.Body)
	}
}

func TestStatsFile(t *testing.T) {
	dir := t.TempDir()
	w := newWorld(t, func(o *Options) { o.StatsDir, o.Stack, o.StatsInterval = dir, "xylem-canary", time.Hour })
	gone := w.subscribe("de", "exams")
	w.subscribe("de", "exams")
	gone.answer(http.StatusGone)
	w.announce(AnnounceRequest{Topic: "exams", Texts: textsOf("de", "Prüfungen", "")})
	w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", `{`)
	if err := w.srv.writeStats(); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(dir, StatsFile))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, line := range []string{
		`xylem_up{stack="xylem-canary"} 1`,
		`xylem_build_info{stack="xylem-canary",version="test",build=""} 1`,
		`xylem_subscriptions{stack="xylem-canary"} 1`,
		`xylem_subscriptions_max{stack="xylem-canary"} 200000`,
		`xylem_subscriptions_changed_total{stack="xylem-canary",event="added",service="other"} 2`,
		`xylem_subscriptions_changed_total{stack="xylem-canary",event="gone",service="other"} 1`,
		`xylem_pushes_total{stack="xylem-canary",service="other",result="sent"} 1`,
		`xylem_pushes_total{stack="xylem-canary",service="other",result="gone"} 1`,
		`xylem_push_seconds_count{stack="xylem-canary",service="other"} 2`,
		`xylem_announcements_total{stack="xylem-canary",event="made"} 1`,
		`xylem_announcements_total{stack="xylem-canary",event="done"} 1`,
		`xylem_announcements_pending{stack="xylem-canary",state="waiting"} 0`,
		`xylem_retries_waiting{stack="xylem-canary"} 0`,
		`xylem_rejections_total{stack="xylem-canary",reason="invalid"} 1`,
	} {
		if !strings.Contains(text, line+"\n") {
			t.Errorf("the stats file lacks %s", line)
		}
	}
	if strings.Contains(text, "exams") || strings.Contains(text, "/wpush/") {
		t.Error("a topic or an endpoint is a label of the stats file")
	}
	if err := w.srv.Close(); err != nil {
		t.Fatal(err)
	}
	data, _ = os.ReadFile(filepath.Join(dir, StatsFile))
	if !strings.Contains(string(data), `xylem_up{stack="xylem-canary"} 0`) {
		t.Error("the last stats file does not say xylem_up 0")
	}
}

// What serve's own client may connect to, whatever a name resolved to.
func TestOnlyPublicAddressesAreDialled(t *testing.T) {
	for address, ok := range map[string]bool{
		"142.250.185.74:443":      true,
		"17.188.143.34:443":       true,
		"142.250.185.74:80":       false,
		"142.250.185.74:8443":     false,
		"127.0.0.1:443":           false,
		"10.0.3.7:443":            false,
		"172.17.0.1:443":          false,
		"192.168.1.1:443":         false,
		"169.254.169.254:443":     false,
		"100.64.0.1:443":          false,
		"0.0.0.0:443":             false,
		"224.0.0.1:443":           false,
		"255.255.255.255:443":     false,
		"[::1]:443":               false,
		"[2a00:1450:4001::5]:443": false, // IPv6: none, as Phloem
		"[::ffff:127.0.0.1]:443":  false,
		"[fd00::1]:443":           false,
		"fcm.googleapis.com:443":  false, // a name: the dialer hands addresses
	} {
		if err := publicOnly("tcp", address, nil); (err == nil) != ok {
			t.Errorf("%s: %v, want taken %v", address, err, ok)
		}
	}
	if !public(netip.MustParseAddr("::ffff:8.8.8.8")) {
		t.Error("an IPv4 address written as IPv6 is not public")
	}
}

func TestNewChecksItsOptions(t *testing.T) {
	w := newWorld(t)
	base := func() Options {
		st, err := store.Open(t.TempDir())
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = st.Close() })
		return Options{Store: st, Key: w.srv.key, Site: "https://betula.app"}
	}
	for name, change := range map[string]func(o *Options){
		"no store":            func(o *Options) { o.Store = nil },
		"no key":              func(o *Options) { o.Key = nil },
		"no site":             func(o *Options) { o.Site = "" },
		"a site with a path":  func(o *Options) { o.Site = "https://betula.app/app" },
		"a site over http":    func(o *Options) { o.Site = "http://betula.app" },
		"a contact of words":  func(o *Options) { o.Contact = "the owner" },
		"a contact over http": func(o *Options) { o.Contact = "http://betula.app" },
		"a mailto with space": func(o *Options) { o.Contact = "mailto: someone@betula.app" },
		"an origin's path":    func(o *Options) { o.Origins = []string{"https://betula.app/x"} },
		"a stack's name":      func(o *Options) { o.Stack = `x"y` },
		"a language":          func(o *Options) { o.Lang = "Deutsch" },
	} {
		o := base()
		change(&o)
		if srv, err := New(o); err == nil {
			_ = srv.Close()
			t.Errorf("%s: taken", name)
		}
	}
	o := base()
	o.Contact = "mailto:someone@betula.app"
	srv, err := New(o)
	if err != nil {
		t.Fatal(err)
	}
	if srv.contact != "mailto:someone@betula.app" || srv.site != "https://betula.app" || srv.lang != "de" || srv.max != DefaultMaxSubscriptions {
		t.Fatalf("contact %q site %q lang %q max %d", srv.contact, srv.site, srv.lang, srv.max)
	}
	_ = srv.Close()
}
