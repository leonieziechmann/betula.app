package server

import (
	"crypto/ecdh"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// The live check: one announcement through a real push service, Mozilla's, to a subscription
// made for the check, and what arrives opened again. It needs the internet and a browser, so it
// runs only when asked (docs/xylem/xylem.md, „Checks"):
//
//	dir=$(mktemp -d)
//	node live/autopush.mjs "$dir" &
//	XYLEM_LIVE_DIR="$dir" go test ./internal/server -run TestLiveThroughMozilla -count=1 -v
//
// The script is the browser's side (it subscribes, and hands back what the service delivers);
// this test is Xylem: the server as serve runs it, with its own checks of the endpoint. Only its
// client is Go's default one instead of Xylem's own, so that the check also runs where the way
// out is a proxy (Xylem's own goes through none).
func TestLiveThroughMozilla(t *testing.T) {
	dir := os.Getenv("XYLEM_LIVE_DIR")
	if dir == "" {
		t.Skip("XYLEM_LIVE_DIR is not set: the live check runs only when asked")
	}
	wait := func(name string) string {
		t.Helper()
		for deadline := time.Now().Add(2 * time.Minute); time.Now().Before(deadline); time.Sleep(200 * time.Millisecond) {
			if data, err := os.ReadFile(filepath.Join(dir, name)); err == nil && len(data) > 0 {
				return strings.TrimSpace(string(data))
			}
		}
		t.Fatalf("no %s/%s: is live/autopush.mjs running?", dir, name)
		return ""
	}
	st, err := store.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	key, err := webpush.NewKey()
	if err != nil {
		t.Fatal(err)
	}
	srv, err := New(Options{
		Store: st, Key: key, Site: "https://betula.app", Version: "live",
		Client: &http.Client{Timeout: 30 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }},
		Limits: Limits{TickEvery: time.Hour},
	})
	if err != nil {
		t.Fatal(err)
	}
	defer srv.Close()
	h := srv.Handler()
	if err := os.WriteFile(filepath.Join(dir, "vapid"), []byte(key.Public()), 0o600); err != nil {
		t.Fatal(err)
	}
	endpoint := wait("endpoint")
	t.Logf("subscribed at %s", strings.SplitN(strings.TrimPrefix(endpoint, "https://"), "/", 2)[0])

	private, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	auth := make([]byte, webpush.AuthLen)
	if _, err := rand.Read(auth); err != nil {
		t.Fatal(err)
	}
	w := &world{t: t, srv: srv, st: st, h: h}
	if rec := w.do(http.MethodPut, "/push/v1/subscription", "203.0.113.9", sub(endpoint, private, auth, "de", "exams")); rec.Code != http.StatusNoContent {
		t.Fatalf("Xylem does not take the endpoint of Mozilla's push service: %d %s", rec.Code, rec.Body)
	}
	out := w.announce(AnnounceRequest{Topic: "exams", Key: "live", For: "5m", Link: "/studyplan",
		Texts: textsOf("de", "Die Prüfungsphase beginnt", "Ab heute laufen die Prüfungen. Viel Erfolg!")})
	a := w.state(out.Announcement.ID)
	if a.State != store.Done || a.Sent != 1 {
		t.Fatalf("Mozilla's push service did not take it: %s, sent %d gone %d failed %d", a.State, a.Sent, a.Gone, a.Failed)
	}
	t.Log("the push service took the message (201)")

	var delivered struct {
		Data    string
		Headers map[string]string
	}
	if err := json.Unmarshal([]byte(wait("notification")), &delivered); err != nil {
		t.Fatal(err)
	}
	body, err := base64.RawURLEncoding.DecodeString(strings.TrimRight(delivered.Data, "="))
	if err != nil {
		t.Fatalf("what was delivered is no base64url: %v", err)
	}
	plain, err := webpush.Open(body, private, auth)
	if err != nil {
		t.Fatalf("what was delivered does not open with the subscription's keys: %v (headers %v)", err, delivered.Headers)
	}
	var m message
	if err := json.Unmarshal(plain, &m); err != nil {
		t.Fatal(err)
	}
	if m.WebPush != 8030 || m.Notification.Title != "Die Prüfungsphase beginnt" || m.Notification.Navigate != "https://betula.app/studyplan" ||
		m.Notification.Tag != "live" || m.Notification.Data.Topic != "exams" {
		t.Fatalf("delivered: %s", plain)
	}
	if enc := delivered.Headers["encoding"]; enc != "" && enc != "aes128gcm" {
		t.Fatalf("delivered with the encoding %q", enc)
	}
	t.Logf("delivered and opened: %s", plain)
}
