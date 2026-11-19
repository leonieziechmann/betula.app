package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/server"
	"github.com/leonieziechmann/betula/xylem/internal/store"
)

// The commands that ask the instance of this machine: over loopback, where it answers them and
// nobody else (docs/xylem/xylem.md, „Commands"). In the container:
//
//	docker exec <container> /bin/xylem announce …

// clientTimeout bounds a command's request: the container's health check gives 5 s.
// feedTimeout is `xylem feed`'s: the instance reads the site's feed while the command waits,
// and gives that read 20 s itself (server.Limits.FeedTimeout).
const (
	clientTimeout = 5 * time.Second
	feedTimeout   = 25 * time.Second
)

// instance is where the commands find the instance of this machine: --instance, else from
// XYLEM_ADDR (the address serve listens on; every address means loopback), else the default.
func instance(fs *flag.FlagSet) *string {
	return fs.String("instance", envOr("XYLEM_INSTANCE", localURL(envOr("XYLEM_ADDR", defaultAddr))),
		"The running instance of this machine (env XYLEM_INSTANCE; else from XYLEM_ADDR)")
}

func localURL(addr string) string {
	host, port, err := net.SplitHostPort(addr)
	if err != nil {
		return "http://" + defaultAddr
	}
	if host == "" || host == "0.0.0.0" || host == "::" {
		host = "127.0.0.1"
	}
	return "http://" + net.JoinHostPort(host, port)
}

// call asks the local instance; it never goes through a proxy.
func call(ctx context.Context, method, url string, body any) (int, []byte, error) {
	timeout := clientTimeout
	if strings.HasSuffix(url, "/feed") {
		timeout = feedTimeout
	}
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	var reader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return 0, nil, err
		}
		reader = bytes.NewReader(data)
	}
	req, err := http.NewRequestWithContext(ctx, method, url, reader)
	if err != nil {
		return 0, nil, err
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	resp, err := (&http.Client{Transport: &http.Transport{Proxy: nil}}).Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(io.LimitReader(resp.Body, 4<<20))
	return resp.StatusCode, data, err
}

// ask calls the local instance and reads its answer into out; false after it said what failed.
func ask(ctx context.Context, method, url string, body, out any, want ...int) (int, bool) {
	status, data, err := call(ctx, method, url, body)
	if err != nil {
		if ctx.Err() != nil {
			fmt.Fprintln(stderr, "Interrupted.")
			return 130, false
		}
		fmt.Fprintln(stderr, "Error:", err)
		return 1, false
	}
	for _, w := range want {
		if status == w {
			if out != nil {
				if err := json.Unmarshal(data, out); err != nil {
					fmt.Fprintf(stderr, "Error: %s answered what does not read: %v\n", url, err)
					return 1, false
				}
			}
			return status, true
		}
	}
	var problem struct{ Error, Detail string }
	_ = json.Unmarshal(data, &problem)
	switch {
	case problem.Detail != "":
		fmt.Fprintln(stderr, "Error:", problem.Detail)
	case problem.Error != "":
		fmt.Fprintf(stderr, "Error: %s answered %d: %s\n", url, status, problem.Error)
	default:
		fmt.Fprintf(stderr, "Error: %s answered %d: %s\n", url, status, strings.TrimSpace(string(data)))
	}
	return 1, false
}

// runHealthcheck asks the local instance whether it is live (GET /healthz), so that the image
// needs no curl: exit 0 for a 200, else 1.
func runHealthcheck(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("healthcheck", flag.ContinueOnError)
	base := instance(fs)
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	status, body, err := call(ctx, http.MethodGet, *base+"/healthz", nil)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	fmt.Fprintln(stdout, strings.TrimSpace(string(body)))
	if status != http.StatusOK {
		return 1
	}
	return 0
}

// runStatus prints the status of the local instance as one JSON object.
func runStatus(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	base := instance(fs)
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	var st json.RawMessage
	if code, ok := ask(ctx, http.MethodGet, *base+"/status", nil, &st, http.StatusOK); !ok {
		return code
	}
	fmt.Fprintln(stdout, strings.TrimSpace(string(st)))
	return 0
}

// runKey prints the public half of the VAPID key.
func runKey(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("key", flag.ContinueOnError)
	base := instance(fs)
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	var out struct{ Key string }
	if code, ok := ask(ctx, http.MethodGet, *base+"/push/v1/key", nil, &out, http.StatusOK); !ok {
		return code
	}
	fmt.Fprintln(stdout, out.Key)
	return 0
}

// texts collects a flag given once per language: "value" for the default language, "en=value"
// for another.
type texts struct {
	def    string
	values map[string]string
}

var langPrefix = regexp.MustCompile(`^([a-z]{2,3}(?:-[a-z0-9]{2,8})?)=`)

func (t *texts) String() string { return "" }

func (t *texts) Set(value string) error {
	lang := ""
	if m := langPrefix.FindStringSubmatch(value); m != nil {
		lang, value = m[1], value[len(m[0]):]
	}
	if _, twice := t.values[lang]; twice {
		if lang == "" {
			return errors.New("given twice for the default language (another language: lang=…)")
		}
		return fmt.Errorf("given twice for %q", lang)
	}
	if t.values == nil {
		t.values = map[string]string{}
	}
	t.values[lang] = value
	return nil
}

// The forms --at takes beside RFC 3339, in the time zone of the machine (TZ).
var localTimes = []string{"2006-01-02 15:04", "2006-01-02T15:04", "2006-01-02 15:04:05", "2006-01-02T15:04:05"}

func parseAt(text string) (time.Time, error) {
	if t, err := time.Parse(time.RFC3339, text); err == nil {
		return t, nil
	}
	for _, layout := range localTimes {
		if t, err := time.ParseInLocation(layout, text, time.Local); err == nil {
			return t, nil
		}
	}
	return time.Time{}, fmt.Errorf("--at %q: a time as 2026-12-21 07:00 (this machine's zone) or 2026-12-21T07:00:00+01:00", text)
}

// runAnnounce makes an announcement: everybody who listens to its topic is told, now or at
// --at, in their language.
func runAnnounce(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("announce", flag.ContinueOnError)
	base := instance(fs)
	topic := fs.String("topic", "", "Who hears it: the subscriptions that listen to this topic (exams, semester, breaks …)")
	var title, body, link texts
	fs.Var(&title, "title", "The notification's title in the default language; once more per other language as lang=title (--title en=\"Exam period\")")
	fs.Var(&body, "body", "Its text, in the same way")
	fs.Var(&link, "link", "The path on the site it opens (default /); for a language whose page has another address, lang=/path")
	at := fs.String("at", "", "When it is told: 2026-12-21 07:00 in this machine's time zone, or RFC 3339; default now")
	life := fs.String("for", "", "How long it is told: a device that was off hears it when it is back within this, nobody after (default 24h)")
	urgency := fs.String("urgency", "", "How pressing the push message is: very-low, low, normal (default), high (wakes a sleeping phone)")
	key := fs.String("key", "", "A name of its own (exams-2027-02): made again under it, it is not told twice")
	dry := fs.Bool("dry-run", false, "Check it and say whom it would reach; tell nobody")
	asJSON := fs.Bool("json", false, "Print the answer as JSON")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	req := server.AnnounceRequest{Key: *key, Topic: *topic, For: *life, Urgency: *urgency, Link: link.values[""], Dry: *dry, Texts: map[string]store.Text{}}
	if *at != "" {
		t, err := parseAt(*at)
		if err != nil {
			fmt.Fprintln(stderr, "Error:", err)
			return 2
		}
		req.At = &t
	}
	if *topic == "" || title.values[""] == "" {
		fmt.Fprintln(stderr, "Error: --topic and --title are needed")
		fs.Usage()
		return 2
	}
	// The texts go by language; "" is the default language, which the instance knows.
	langs := map[string]bool{}
	for _, t := range []texts{title, body, link} {
		for lang := range t.values {
			langs[lang] = true
		}
	}
	var status server.Status
	if code, ok := ask(ctx, http.MethodGet, *base+"/status", nil, &status, http.StatusOK); !ok {
		return code
	}
	for lang := range langs {
		name := lang
		if name == "" {
			name = status.Lang
		}
		if _, twice := req.Texts[name]; twice {
			fmt.Fprintf(stderr, "Error: %q is the default language: its texts go without %s=\n", name, name)
			return 2
		}
		text := store.Text{Title: title.values[lang], Body: body.values[lang]}
		if lang != "" {
			text.Link = link.values[lang]
		}
		if text.Title == "" {
			fmt.Fprintf(stderr, "Error: %q has a body or a link, and no --title %s=…\n", name, name)
			return 2
		}
		req.Texts[name] = text
	}
	var out server.AnnounceResponse
	if code, ok := ask(ctx, http.MethodPost, *base+"/announcements", req, &out, http.StatusOK, http.StatusCreated); !ok {
		return code
	}
	if *asJSON {
		data, _ := json.Marshal(out)
		fmt.Fprintln(stdout, string(data))
		return 0
	}
	a := out.Announcement
	switch {
	case *dry:
		fmt.Fprintf(stdout, "Nothing is told (a dry run). It would be: topic %s, from %s until %s.\n", a.Topic, local(a.At), local(a.Until))
	case !out.Made:
		fmt.Fprintf(stdout, "Announcement %d has the key %q already (%s): nothing new is told.\n", a.ID, a.Key, a.State)
	default:
		fmt.Fprintf(stdout, "Announcement %d: topic %s, told from %s until %s.\n", a.ID, a.Topic, local(a.At), local(a.Until))
	}
	fmt.Fprintf(stdout, "Listening to %s now: %s.\n", a.Topic, reach(out.Reach))
	return 0
}

func local(t time.Time) string { return t.Local().Format("2006-01-02 15:04 MST") }

// reach says how many listen, by language.
func reach(by map[string]int64) string {
	if len(by) == 0 {
		return "nobody"
	}
	langs := make([]string, 0, len(by))
	var total int64
	for lang, n := range by {
		langs = append(langs, lang)
		total += n
	}
	sort.Strings(langs)
	parts := make([]string, len(langs))
	for i, lang := range langs {
		parts[i] = fmt.Sprintf("%s %d", lang, by[lang])
	}
	return fmt.Sprintf("%d subscriptions (%s)", total, strings.Join(parts, ", "))
}

// runAnnouncements lists the newest announcements.
func runAnnouncements(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("announcements", flag.ContinueOnError)
	base := instance(fs)
	limit := fs.Int("limit", 20, "How many, the newest first")
	asJSON := fs.Bool("json", false, "Print them as JSON")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	var out struct {
		Lang          string
		Announcements []server.AnnouncementView
	}
	var raw json.RawMessage
	if code, ok := ask(ctx, http.MethodGet, fmt.Sprintf("%s/announcements?limit=%d", *base, *limit), nil, &raw, http.StatusOK); !ok {
		return code
	}
	if *asJSON {
		fmt.Fprintln(stdout, strings.TrimSpace(string(raw)))
		return 0
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	if len(out.Announcements) == 0 {
		fmt.Fprintln(stdout, "No announcements.")
		return 0
	}
	fmt.Fprintf(stdout, "%-5s %-10s %-8s %-12s %-22s %6s %6s %6s  %s\n", "id", "state", "by", "topic", "at", "sent", "gone", "failed", "title")
	for _, a := range out.Announcements {
		fmt.Fprintf(stdout, "%-5d %-10s %-8s %-12s %-22s %6d %6d %6d  %s\n", a.ID, a.State, a.Source, a.Topic, local(a.At), a.Sent, a.Gone, a.Failed, a.Texts[out.Lang].Title)
	}
	return 0
}

// runCancel ends an announcement that is not finished.
func runCancel(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("cancel", flag.ContinueOnError)
	base := instance(fs)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: xylem cancel [flags] <id>")
		fs.PrintDefaults()
	}
	if code, ok := parse(fs, args, 1, 1); !ok {
		return code
	}
	var out struct{ Announcement server.AnnouncementView }
	status, data, err := call(ctx, http.MethodDelete, *base+"/announcements/"+fs.Arg(0), nil)
	switch {
	case err != nil:
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	case status == http.StatusNotFound:
		fmt.Fprintf(stderr, "Error: no announcement %s\n", fs.Arg(0))
		return 1
	case status == http.StatusConflict:
		fmt.Fprintf(stderr, "Error: announcement %s is finished already\n", fs.Arg(0))
		return 1
	case status != http.StatusOK || json.Unmarshal(data, &out) != nil:
		fmt.Fprintf(stderr, "Error: the instance answered %d: %s\n", status, strings.TrimSpace(string(data)))
		return 1
	}
	a := out.Announcement
	fmt.Fprintf(stdout, "Announcement %d (%s) is cancelled; %d subscriptions had it already.\n", a.ID, a.Topic, a.Sent)
	return 0
}

// runFeed reads the feed now instead of at its next turn, and says what the read changed.
func runFeed(ctx context.Context, args []string) int {
	fs := flag.NewFlagSet("feed", flag.ContinueOnError)
	base := instance(fs)
	asJSON := fs.Bool("json", false, "Print the answer as JSON")
	if code, ok := parse(fs, args, 0, 0); !ok {
		return code
	}
	var raw json.RawMessage
	if code, ok := ask(ctx, http.MethodPost, *base+"/feed", struct{}{}, &raw, http.StatusOK); !ok {
		return code
	}
	if *asJSON {
		fmt.Fprintln(stdout, strings.TrimSpace(string(raw)))
		return 0
	}
	var report server.FeedReport
	if err := json.Unmarshal(raw, &report); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 1
	}
	switch {
	case report.Unchanged:
		fmt.Fprintln(stdout, "The feed is what it was at the last read.")
		return 0
	case report.Baseline:
		fmt.Fprintf(stdout, "The feed names %d announcements. This was its first read: what has no time of its own is old news and is skipped.\n", report.Entries)
	default:
		fmt.Fprintf(stdout, "The feed names %d announcements.\n", report.Entries)
	}
	if len(report.Changes) == 0 {
		fmt.Fprintln(stdout, "Nothing changed.")
	}
	for _, c := range report.Changes {
		line := fmt.Sprintf("  %-10s %s", c.What, c.Key)
		if c.At != nil {
			line += "  (" + local(*c.At) + ")"
		}
		if c.Detail != "" {
			line += "  " + c.Detail
		}
		fmt.Fprintln(stdout, line)
	}
	return 0
}
