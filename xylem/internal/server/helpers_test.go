package server

import (
	"bytes"
	"crypto/ecdh"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// clock is the time of a test: it stands still until the test moves it.
type clock struct {
	mu  sync.Mutex
	now time.Time
}

func (c *clock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.now
}

func (c *clock) Add(d time.Duration) {
	c.mu.Lock()
	c.now = c.now.Add(d)
	c.mu.Unlock()
}

// start is when every test begins: a Monday morning in the lecture period.
var start = time.Date(2026, 11, 23, 8, 0, 0, 0, time.UTC)

// received is one push message as a browser's push service got it and its browser opened it.
type received struct {
	Message message
	Raw     string
	TTL     int
	Urgency string
	Token   string // the VAPID token it came with
}

// browser is one browser with a push subscription at the push service of the test.
type browser struct {
	push    *pushService
	path    string
	private *ecdh.PrivateKey
	auth    []byte

	mu      sync.Mutex
	got     []received
	answers []int // the statuses its next requests are answered with; then 201
	after   int   // the Retry-After of every answer that is no 201, in seconds
	hits    int
}

func (b *browser) endpoint() string { return b.push.srv.URL + b.path }

func (b *browser) messages() []received {
	b.mu.Lock()
	defer b.mu.Unlock()
	return append([]received(nil), b.got...)
}

func (b *browser) requests() int {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.hits
}

// answer sets the statuses the browser's next requests are answered with.
func (b *browser) answer(statuses ...int) {
	b.mu.Lock()
	b.answers = statuses
	b.mu.Unlock()
}

// pushService plays a push service: it takes a request only as a real one would (RFC 8030,
// RFC 8291, RFC 8292), and opens what it gets with the browser's keys.
type pushService struct {
	t   *testing.T
	srv *httptest.Server
	key *webpush.Key // Xylem's
	now func() time.Time

	mu       sync.Mutex
	browsers map[string]*browser
	next     int
}

func newPushService(t *testing.T, key *webpush.Key, now func() time.Time) *pushService {
	p := &pushService{t: t, key: key, now: now, browsers: map[string]*browser{}}
	p.srv = httptest.NewServer(http.HandlerFunc(p.serve))
	t.Cleanup(p.srv.Close)
	return p
}

func (p *pushService) browser() *browser {
	p.t.Helper()
	private, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		p.t.Fatal(err)
	}
	auth := make([]byte, webpush.AuthLen)
	if _, err := rand.Read(auth); err != nil {
		p.t.Fatal(err)
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	p.next++
	b := &browser{push: p, path: fmt.Sprintf("/wpush/v2/device-%d", p.next), private: private, auth: auth}
	p.browsers[b.path] = b
	return b
}

func (p *pushService) serve(w http.ResponseWriter, r *http.Request) {
	p.mu.Lock()
	b := p.browsers[r.URL.Path]
	p.mu.Unlock()
	if b == nil {
		w.WriteHeader(http.StatusNotFound)
		return
	}
	body, _ := io.ReadAll(r.Body)
	b.mu.Lock()
	b.hits++
	status := http.StatusCreated
	if len(b.answers) > 0 {
		status, b.answers = b.answers[0], b.answers[1:]
	}
	after := b.after
	b.mu.Unlock()

	// What every push service checks before it looks at the subscription's state.
	problem := ""
	ttl, err := strconv.Atoi(r.Header.Get("TTL"))
	claims, public, verr := webpush.Verify(r.Header.Get("Authorization"))
	switch {
	case r.Method != http.MethodPost:
		problem = "method " + r.Method
	case r.Header.Get("Content-Encoding") != "aes128gcm":
		problem = "Content-Encoding " + r.Header.Get("Content-Encoding")
	case err != nil || ttl < 0:
		problem = "TTL " + r.Header.Get("TTL")
	case !urgencies[r.Header.Get("Urgency")]:
		problem = "Urgency " + r.Header.Get("Urgency")
	case verr != nil:
		problem = "Authorization: " + verr.Error()
	case base64.RawURLEncoding.EncodeToString(public) != p.key.Public():
		problem = "the token's key is not the subscription's"
	case claims.Audience != p.srv.URL:
		problem = "aud " + claims.Audience
	case !claims.Expires.After(p.now()) || claims.Expires.After(p.now().Add(24*time.Hour)):
		problem = "exp " + claims.Expires.String()
	case claims.Subject == "":
		problem = "no sub"
	case len(body) > 4096:
		problem = "a body of " + strconv.Itoa(len(body)) + " bytes"
	}
	if problem != "" {
		p.t.Errorf("the push service would refuse a request to %s: %s", r.URL.Path, problem)
		w.WriteHeader(http.StatusBadRequest)
		return
	}
	if status != http.StatusCreated {
		if after > 0 {
			w.Header().Set("Retry-After", strconv.Itoa(after))
		}
		w.WriteHeader(status)
		return
	}
	plain, err := webpush.Open(body, b.private, b.auth)
	if err != nil {
		p.t.Errorf("a message to %s does not open: %v", r.URL.Path, err)
		w.WriteHeader(http.StatusBadRequest)
		return
	}
	got := received{Raw: string(plain), TTL: ttl, Urgency: r.Header.Get("Urgency"), Token: r.Header.Get("Authorization")}
	if err := json.Unmarshal(plain, &got.Message); err != nil {
		p.t.Errorf("a message to %s is no JSON: %v", r.URL.Path, err)
	}
	b.mu.Lock()
	b.got = append(b.got, got)
	b.mu.Unlock()
	w.WriteHeader(http.StatusCreated)
}

// world is a Xylem with a push service and a clock of the test's.
type world struct {
	t     *testing.T
	srv   *Server
	st    *store.Store
	push  *pushService
	clock *clock
	h     http.Handler
}

// newWorld starts a Xylem for https://betula.test whose push service is the test's. change
// may change the options before it starts.
func newWorld(t *testing.T, change ...func(*Options)) *world {
	t.Helper()
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	key, err := webpush.NewKey()
	if err != nil {
		t.Fatal(err)
	}
	c := &clock{now: start}
	push := newPushService(t, key, c.Now)
	opt := Options{
		Store: st, Key: key, Site: "https://betula.test", Version: "test", Now: c.Now,
		Hosts: []string{"127.0.0.1"}, Insecure: true, Client: push.srv.Client(),
		// The sender runs when the test says so (work), and when an announcement wakes it.
		Limits: Limits{TickEvery: time.Hour, SweepEvery: time.Hour, Backoff: []time.Duration{time.Minute, 5 * time.Minute}},
	}
	for _, f := range change {
		f(&opt)
	}
	srv, err := New(opt)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = srv.Close()
		_ = st.Close()
	})
	return &world{t: t, srv: srv, st: st, push: push, clock: c, h: srv.Handler()}
}

// do sends a request as a browser behind Traefik would (remote: the address Traefik saw), or
// as a command of this machine (remote "").
func (w *world) do(method, path, remote string, body any, header ...string) *httptest.ResponseRecorder {
	w.t.Helper()
	var reader io.Reader
	switch b := body.(type) {
	case nil:
	case string:
		reader = strings.NewReader(b)
	default:
		data, err := json.Marshal(b)
		if err != nil {
			w.t.Fatal(err)
		}
		reader = bytes.NewReader(data)
	}
	req := httptest.NewRequest(method, path, reader)
	req.Host = "betula.test"
	req.RemoteAddr = "127.0.0.1:40000"
	if remote != "" {
		req.RemoteAddr = "10.0.3.7:40000" // Traefik, on the overlay network
		req.Header.Set("X-Real-Ip", remote)
	}
	for i := 0; i+1 < len(header); i += 2 {
		req.Header.Set(header[i], header[i+1])
	}
	rec := httptest.NewRecorder()
	w.h.ServeHTTP(rec, req)
	return rec
}

// sub is the body of PUT /push/v1/subscription, as a browser builds it from its subscription.
func sub(endpoint string, private *ecdh.PrivateKey, auth []byte, lang string, topics ...string) map[string]any {
	if topics == nil {
		topics = []string{}
	}
	return map[string]any{
		"endpoint":       endpoint,
		"expirationTime": nil,
		"keys": map[string]string{
			"p256dh": base64.RawURLEncoding.EncodeToString(private.PublicKey().Bytes()),
			"auth":   base64.RawURLEncoding.EncodeToString(auth),
		},
		"lang":   lang,
		"topics": topics,
	}
}

// subscribe makes a browser and subscribes it.
func (w *world) subscribe(lang string, topics ...string) *browser {
	w.t.Helper()
	b := w.push.browser()
	w.put(b, lang, topics...)
	return b
}

func (w *world) put(b *browser, lang string, topics ...string) {
	w.t.Helper()
	if rec := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", sub(b.endpoint(), b.private, b.auth, lang, topics...)); rec.Code != http.StatusNoContent {
		w.t.Fatalf("subscribe: %d %s", rec.Code, rec.Body)
	}
}

// textsOf are the texts of an announcement: a language, its title and its body, three by three.
func textsOf(triples ...string) map[string]store.Text {
	out := map[string]store.Text{}
	for i := 0; i+2 < len(triples); i += 3 {
		out[triples[i]] = store.Text{Title: triples[i+1], Body: triples[i+2]}
	}
	return out
}

// announce makes an announcement through the commands' side and lets the sender do what is due.
func (w *world) announce(req AnnounceRequest) AnnounceResponse {
	w.t.Helper()
	rec := w.do(http.MethodPost, "/announcements", "", req)
	if rec.Code != http.StatusCreated && rec.Code != http.StatusOK {
		w.t.Fatalf("announce: %d %s", rec.Code, rec.Body)
	}
	var out AnnounceResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &out); err != nil {
		w.t.Fatal(err)
	}
	w.srv.work()
	return out
}

// state reads an announcement.
func (w *world) state(id int64) store.Announcement {
	w.t.Helper()
	a, err := w.st.Announcement(w.t.Context(), id)
	if err != nil {
		w.t.Fatal(err)
	}
	return a
}

// counts checks what an announcement has come to.
func (w *world) counts(id int64, state string, sent, gone, failed int64) {
	w.t.Helper()
	a := w.state(id)
	if a.State != state || a.Sent != sent || a.Gone != gone || a.Failed != failed {
		w.t.Fatalf("announcement %d: %s sent %d gone %d failed %d; want %s %d %d %d", id, a.State, a.Sent, a.Gone, a.Failed, state, sent, gone, failed)
	}
}

func errorOf(rec *httptest.ResponseRecorder) string {
	var body struct{ Error, Detail string }
	_ = json.Unmarshal(rec.Body.Bytes(), &body)
	if body.Detail != "" {
		return body.Error + ": " + body.Detail
	}
	return body.Error
}
