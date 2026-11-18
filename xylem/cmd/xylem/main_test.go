package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"log/slog"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/server"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/version"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// capture replaces stdout and stderr for one command.
func capture(t *testing.T) (out, errOut *bytes.Buffer) {
	t.Helper()
	out, errOut = new(bytes.Buffer), new(bytes.Buffer)
	oldOut, oldErr := stdout, stderr
	stdout, stderr = out, errOut
	t.Cleanup(func() { stdout, stderr = oldOut, oldErr })
	return out, errOut
}

func TestHelpVersionAndUnknownCommands(t *testing.T) {
	out, _ := capture(t)
	if code := run([]string{"help"}); code != 0 || !strings.Contains(out.String(), "announce") || !strings.Contains(out.String(), version.Version) {
		t.Errorf("help: %d %s", code, out)
	}
	out.Reset()
	if code := run([]string{"version"}); code != 0 || !strings.HasPrefix(out.String(), "Xylem "+version.Version) {
		t.Errorf("version: %d %s", code, out)
	}
	_, errOut := capture(t)
	if code := run([]string{"dance"}); code != 2 || !strings.Contains(errOut.String(), "Unknown command") {
		t.Errorf("unknown: %d %s", code, errOut)
	}
	if code := run([]string{"serve", "--no-such-flag"}); code != 2 {
		t.Errorf("a wrong flag: %d", code)
	}
	if code := run([]string{"serve", "extra"}); code != 2 {
		t.Errorf("an argument: %d", code)
	}
	if code := run([]string{"serve", "--help"}); code != 0 {
		t.Errorf("serve --help: %d", code)
	}
	if code := run([]string{"cancel"}); code != 2 {
		t.Errorf("cancel without an id: %d", code)
	}
}

func freeAddr(t *testing.T) string {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer ln.Close()
	return ln.Addr().String()
}

// The one test that runs serve (its metrics live in the process's registry, once), with every
// command against it. Nothing in it is due while it runs: no request leaves the machine.
func TestServeAndItsCommands(t *testing.T) {
	data, stats := t.TempDir(), t.TempDir()
	addr := freeAddr(t)
	// The site's feed of what Betula announces by itself: nothing at first.
	var feedMu sync.Mutex
	feed := `{"announcements":[]}`
	site := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		feedMu.Lock()
		defer feedMu.Unlock()
		_, _ = w.Write([]byte(feed))
	}))
	defer site.Close()
	t.Setenv("XYLEM_ADDR", addr) // how the commands find the instance, as in the image
	t.Setenv("TZ", "UTC")
	capture(t)
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan int, 1)
	go func() {
		done <- runServe(ctx, []string{"--data", data, "--site", "https://betula.app", "--stats-dir", stats, "--stats-interval", "50ms",
			"--stack", "xylem-test", "--origins", "https://betula.app", "--max-subscriptions", "1000", "--log-format", "text",
			"--feed", site.URL + "/api/announcements", "--feed-interval", "1h"})
	}()
	deadline := time.Now().Add(10 * time.Second)
	for {
		resp, err := http.Get("http://" + addr + "/healthz")
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				break
			}
		}
		if time.Now().After(deadline) {
			t.Fatalf("serve did not come up: %v", err)
		}
		time.Sleep(20 * time.Millisecond)
	}
	out, errOut := capture(t)
	command := func(args ...string) (int, string) {
		t.Helper()
		out.Reset()
		errOut.Reset()
		code := run(args)
		return code, out.String() + errOut.String()
	}
	if code, text := command("healthcheck"); code != 0 || strings.TrimSpace(text) != "ok" {
		t.Errorf("healthcheck: %d %q", code, text)
	}
	code, text := command("status")
	var st server.Status
	if err := json.Unmarshal([]byte(text), &st); code != 0 || err != nil || st.Version != version.Version || st.Site != "https://betula.app" ||
		st.Lang != "de" || st.Max != 1000 || st.Stack != "xylem-test" {
		t.Fatalf("status: %d %s %v", code, text, err)
	}
	if code, text := command("key"); code != 0 || strings.TrimSpace(text) != st.Key || len(st.Key) != 87 {
		t.Errorf("key: %d %q, the status says %q", code, text, st.Key)
	}

	if code, text := command("announcements"); code != 0 || !strings.Contains(text, "No announcements") {
		t.Errorf("an empty list: %d %s", code, text)
	}
	code, text = command("announce", "--topic", "exams", "--title", "Die Prüfungsphase beginnt", "--body", "Viel Erfolg!",
		"--title", "en=Exam period begins", "--link", "/studyplan", "--link", "en=/en/studyplan", "--dry-run")
	if code != 0 || !strings.Contains(text, "a dry run") || !strings.Contains(text, "Listening to exams now: nobody") {
		t.Errorf("dry run: %d %s", code, text)
	}
	code, text = command("announce", "--topic", "exams", "--title", "Die Prüfungsphase beginnt", "--title", "en=Exam period begins",
		"--link", "en=/en/studyplan", "--at", "2031-02-03 07:00", "--for", "36h", "--urgency", "high", "--key", "exams-2031-02")
	if code != 0 || !strings.Contains(text, "Announcement 1: topic exams, told from 2031-02-03 07:00 UTC until 2031-02-04 19:00 UTC") {
		t.Fatalf("announce: %d %s", code, text)
	}
	if code, text := command("announce", "--topic", "exams", "--title", "Noch einmal", "--key", "exams-2031-02"); code != 0 || !strings.Contains(text, "already (waiting): nothing new is told") {
		t.Errorf("the same key: %d %s", code, text)
	}
	code, text = command("announce", "--topic", "breaks", "--title", "Weihnachtspause", "--at", "2031-12-22T07:00:00+01:00", "--json")
	var made server.AnnounceResponse
	if err := json.Unmarshal([]byte(text), &made); code != 0 || err != nil || !made.Made || made.Announcement.ID != 2 ||
		!made.Announcement.At.Equal(time.Date(2031, 12, 22, 6, 0, 0, 0, time.UTC)) || made.Announcement.Texts["de"].Title != "Weihnachtspause" {
		t.Errorf("announce --json: %d %s %v", code, text, err)
	}
	for name, args := range map[string][]string{
		"no title":          {"--topic", "exams"},
		"no topic":          {"--title", "x"},
		"a title twice":     {"--topic", "exams", "--title", "a", "--title", "b"},
		"a body alone":      {"--topic", "exams", "--title", "a", "--body", "en=b"},
		"default named":     {"--topic", "exams", "--title", "a", "--title", "de=b"},
		"no time":           {"--topic", "exams", "--title", "a", "--at", "tomorrow"},
		"refused by server": {"--topic", "exams", "--title", "a", "--link", "https://evil.example"},
		"a topic in caps":   {"--topic", "Exams", "--title", "a"},
	} {
		if code, text := command(append([]string{"announce"}, args...)...); code == 0 || text == "" {
			t.Errorf("announce, %s: %d %s", name, code, text)
		}
	}
	code, text = command("announcements")
	lines := strings.Split(strings.TrimSpace(text), "\n")
	if code != 0 || len(lines) != 3 || !strings.Contains(lines[1], "Weihnachtspause") || !strings.Contains(lines[2], "exams") ||
		!strings.Contains(lines[2], "2031-02-03 07:00 UTC") || !strings.Contains(lines[2], "waiting") {
		t.Errorf("announcements: %d\n%s", code, text)
	}
	if code, text := command("announcements", "--json", "--limit", "1"); code != 0 || !strings.Contains(text, `"topic":"breaks"`) || strings.Contains(text, `"topic":"exams"`) {
		t.Errorf("announcements --json: %d %s", code, text)
	}
	if code, text := command("cancel", "1"); code != 0 || !strings.Contains(text, "Announcement 1 (exams) is cancelled") {
		t.Errorf("cancel: %d %s", code, text)
	}
	if code, text := command("cancel", "1"); code != 1 || !strings.Contains(text, "finished already") {
		t.Errorf("cancel again: %d %s", code, text)
	}
	if code, text := command("cancel", "77"); code != 1 || !strings.Contains(text, "no announcement 77") {
		t.Errorf("cancel of none: %d %s", code, text)
	}
	if code, _ := command("status", "--instance", "http://"+freeAddr(t)); code != 1 {
		t.Errorf("status of nothing: %d", code)
	}

	// The feed: read at the start (it named nothing), and now on asking.
	if code, text := command("feed"); code != 0 || !strings.Contains(text, "The feed names 0 announcements.") || !strings.Contains(text, "Nothing changed.") {
		t.Errorf("feed, nothing named: %d %s", code, text)
	}
	feedMu.Lock()
	feed = `{"announcements":[
		{"key":"exams-2030w-1","topic":"exams","at":"2031-02-10T07:00:00+01:00","for":"12h","link":"/","texts":{"de":{"title":"Die Prüfungsphase beginnt"},"en":{"title":"The exam period begins","link":"/en"}}},
		{"key":"dates-2031s","topic":"semester","texts":{"en":{"title":"Only English"}}}]}`
	feedMu.Unlock()
	code, text = command("feed")
	if code != 0 || !strings.Contains(text, "The feed names 2 announcements.") || !strings.Contains(text, "made       exams-2030w-1  (2031-02-10 06:00 UTC)") ||
		!strings.Contains(text, "invalid    dates-2031s") || !strings.Contains(text, `none in "de"`) {
		t.Errorf("feed: %d\n%s", code, text)
	}
	code, text = command("announcements", "--limit", "1")
	if code != 0 || !strings.Contains(text, "feed") || !strings.Contains(text, "Die Prüfungsphase beginnt") || !strings.Contains(text, " by ") {
		t.Errorf("the list after the feed: %d\n%s", code, text)
	}
	if code, text := command("feed", "--json"); code != 0 || !strings.Contains(text, `"entries":2`) {
		t.Errorf("feed --json: %d %s", code, text)
	}
	code, text = command("status")
	if err := json.Unmarshal([]byte(text), &st); code != 0 || err != nil || st.Feed == nil || st.Feed.URL != site.URL+"/api/announcements" || st.Feed.Entries != 2 || st.Feed.Read == nil {
		t.Errorf("status with a feed: %d %s %v", code, text, err)
	}

	// A problem in the log is counted in the stats file (serve installed the logger).
	slog.Warn("a test's warning", "component", "test", "event", "test.warned")
	prom := filepath.Join(stats, "xylem.prom")
	for deadline := time.Now().Add(5 * time.Second); ; {
		if data, err := os.ReadFile(prom); err == nil && strings.Contains(string(data), `xylem_announcements_pending{stack="xylem-test",state="waiting"} 2`) {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("no stats file")
		}
		time.Sleep(20 * time.Millisecond)
	}
	cancel()
	select {
	case code := <-done:
		if code != 0 {
			t.Errorf("serve ended with %d", code)
		}
	case <-time.After(20 * time.Second):
		t.Fatal("serve did not stop")
	}
	if data, _ := os.ReadFile(prom); !strings.Contains(string(data), `xylem_up{stack="xylem-test"} 0`) ||
		!strings.Contains(string(data), `xylem_log_problems_total{stack="xylem-test",level="WARN",event="test.warned"} 1`) {
		t.Errorf("the last stats file:\n%s", data)
	}

	// The key the browsers subscribed with is in the database: a Xylem started again on it
	// has the same one.
	st2, err := store.Open(data)
	if err != nil {
		t.Fatal(err)
	}
	defer st2.Close()
	text, err = st2.VAPID(context.Background(), func() (string, error) { return "", errors.New("the database has no key") })
	if err != nil {
		t.Fatal(err)
	}
	if key, err := webpush.ParseKey(text); err != nil || key.Public() != st.Key {
		t.Errorf("the key of the database is not the one serve handed out: %v", err)
	}
}

func TestServeRefusesABadConfiguration(t *testing.T) {
	capture(t)
	for name, args := range map[string][]string{
		"no site":        {},
		"a site's path":  {"--site", "https://betula.app/app"},
		"plain http":     {"--site", "http://betula.app"},
		"origin":         {"--site", "https://betula.app", "--origins", "betula.app"},
		"no room":        {"--site", "https://betula.app", "--max-subscriptions", "0"},
		"a push host":    {"--site", "https://betula.app", "--push-hosts", "https://push.example"},
		"a contact":      {"--site", "https://betula.app", "--contact", "me"},
		"log level":      {"--site", "https://betula.app", "--log-level", "loud"},
		"a feed":         {"--site", "https://betula.app", "--feed", "betula.app/api/announcements"},
		"feed too often": {"--site", "https://betula.app", "--feed-interval", "5s"},
	} {
		args = append(args, "--addr", freeAddr(t), "--data", t.TempDir(), "--log-format", "text")
		if code := runServe(context.Background(), args); code != 2 {
			t.Errorf("%s: exit %d, want 2", name, code)
		}
	}
}

func TestWhereTheCommandsLook(t *testing.T) {
	for addr, want := range map[string]string{
		"127.0.0.1:8300": "http://127.0.0.1:8300",
		"0.0.0.0:8300":   "http://127.0.0.1:8300", // the image listens on every address
		":9000":          "http://127.0.0.1:9000",
		"[::]:8300":      "http://127.0.0.1:8300",
		"[::1]:8300":     "http://[::1]:8300",
		"nonsense":       "http://127.0.0.1:8300",
	} {
		if got := localURL(addr); got != want {
			t.Errorf("localURL(%q) = %q, want %q", addr, got, want)
		}
	}
}

func TestTimesAndTexts(t *testing.T) {
	berlin, err := time.LoadLocation("Europe/Berlin")
	if err != nil {
		t.Fatal(err)
	}
	old := time.Local
	time.Local = berlin
	defer func() { time.Local = old }()
	for text, want := range map[string]time.Time{
		"2026-12-21 07:00":          time.Date(2026, 12, 21, 6, 0, 0, 0, time.UTC), // winter time
		"2027-07-19T07:00":          time.Date(2027, 7, 19, 5, 0, 0, 0, time.UTC),  // summer time
		"2026-12-21 07:00:30":       time.Date(2026, 12, 21, 6, 0, 30, 0, time.UTC),
		"2026-12-21T07:00:00Z":      time.Date(2026, 12, 21, 7, 0, 0, 0, time.UTC),
		"2026-12-21T07:00:00+01:00": time.Date(2026, 12, 21, 6, 0, 0, 0, time.UTC),
	} {
		if got, err := parseAt(text); err != nil || !got.Equal(want) {
			t.Errorf("parseAt(%q) = %v, %v; want %v", text, got, err, want)
		}
	}
	for _, text := range []string{"", "07:00", "21.12.2026 07:00", "2026-12-21"} {
		if _, err := parseAt(text); err == nil {
			t.Errorf("parseAt(%q) took it", text)
		}
	}

	var title texts
	if err := title.Set("Die Prüfungsphase beginnt"); err != nil {
		t.Fatal(err)
	}
	if err := title.Set("en=Exam period begins"); err != nil {
		t.Fatal(err)
	}
	if err := title.Set("pt-br=Começa o período"); err != nil {
		t.Fatal(err)
	}
	if title.values[""] != "Die Prüfungsphase beginnt" || title.values["en"] != "Exam period begins" || title.values["pt-br"] != "Começa o período" {
		t.Fatalf("texts %v", title.values)
	}
	if err := title.Set("en=again"); err == nil {
		t.Fatal("a language twice was taken")
	}
	// What only looks like a language is the default's text: a title may hold a "=".
	var body texts
	if err := body.Set("E=mc² wird geprüft"); err != nil || body.values[""] != "E=mc² wird geprüft" {
		t.Fatalf("%v %v", body.values, err)
	}
}
