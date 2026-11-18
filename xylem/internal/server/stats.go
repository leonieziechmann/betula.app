package server

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/metrics"
)

// StatsFile is the name of the stats file in Options.StatsDir. Its temporary twin does not end
// in .prom, so that a reader of *.prom never sees half a file.
const (
	StatsFile = "xylem.prom"
	statsTemp = ".xylem.prom.tmp"
)

// meters are the counters of the stats file; the gauges read the server when it is written.
// Label values come from fixed sets (docs/xylem/xylem.md, „Numbers"): a topic is never a label,
// since anybody may name one.
type meters struct {
	subs          *metrics.Counter   // by event and push service
	pushes        *metrics.Counter   // requests to push services, by service and result
	pushSeconds   *metrics.Histogram // how long they took, by service
	announcements *metrics.Counter   // by event
	rejected      *metrics.Counter   // requests refused, by reason
}

func (s *Server) declare() {
	r := s.reg
	s.m = meters{
		subs: r.NewCounter("xylem_subscriptions_changed_total",
			"Subscriptions, by event (added; updated: put again; removed: by its browser; gone: its push service said it is no more) and push service (google, mozilla, apple, microsoft, other).", "event", "service"),
		pushes: r.NewCounter("xylem_pushes_total",
			"Requests to push services, by service and result (sent; gone: 404 or 410, the subscription is forgotten; retry: no answer, 429 or 5xx, tried again later; gave_up: no attempt or time left; unauthorized: 401 or 403, the service refuses Xylem's token; invalid: 400; too_big: 413; refused: anything else).", "service", "result"),
		pushSeconds: r.NewHistogram("xylem_push_seconds", "How long a request to a push service took, by service.",
			[]float64{0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 15}, "service"),
		announcements: r.NewCounter("xylem_announcements_total",
			"Announcements, by event (made; again: made under a key that was taken; started; done; expired: Xylem did not run while it was due; cancelled).", "event"),
		rejected: r.NewCounter("xylem_rejections_total",
			"Requests of browsers refused, by reason (origin, invalid, too_big, endpoint, keys, lang, topics, slow_down, full).", "reason"),
	}
	r.NewGaugeFunc("xylem_up", "1 while Xylem runs; the last file it writes says 0.", nil,
		func(emit func(float64, ...string)) { emit(metrics.Bool(s.up.Load())) })
	build := s.opt.Build
	if len(build) > 12 {
		build = build[:12]
	}
	r.NewGaugeFunc("xylem_build_info", "Always 1: the version and build (the start of the sha256 of the executable) that run.",
		[]string{"version", "build"}, func(emit func(float64, ...string)) { emit(1, s.opt.Version, build) })
	started := float64(s.started.Unix())
	r.NewGaugeFunc("xylem_start_time_seconds", "When the process started (Unix time).", nil,
		func(emit func(float64, ...string)) { emit(started) })
	r.NewGaugeFunc("xylem_subscriptions", "Subscriptions kept.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.st.Subscriptions())) })
	r.NewGaugeFunc("xylem_subscriptions_max", "How many subscriptions Xylem holds; at it, no new one.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.max)) })
	r.NewGaugeFunc("xylem_announcements_pending", "Announcements that are not finished, by state (waiting, sending, retrying).",
		[]string{"state"}, func(emit func(float64, ...string)) {
			byState, _, err := s.st.Pending(context.Background())
			if err != nil {
				return
			}
			for _, state := range []string{"waiting", "sending", "retrying"} {
				emit(float64(byState[state]), state)
			}
		})
	r.NewGaugeFunc("xylem_retries_waiting", "Subscriptions that wait for another attempt of an announcement.", nil,
		func(emit func(float64, ...string)) {
			if _, n, err := s.st.Pending(context.Background()); err == nil {
				emit(float64(n))
			}
		})
	r.NewGaugeFunc("xylem_stats_time_seconds", "When this file was written (Unix time).", nil,
		func(emit func(float64, ...string)) { emit(float64(time.Now().Unix())) })
}

// writeStats writes the stats file at once: a temporary file, then the rename.
func (s *Server) writeStats() error {
	var buf bytes.Buffer
	if err := s.reg.WriteText(&buf); err != nil {
		return err
	}
	data := withLabel(buf.Bytes(), "stack", s.opt.Stack)
	tmp := filepath.Join(s.opt.StatsDir, statsTemp)
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, filepath.Join(s.opt.StatsDir, StatsFile))
}

// withLabel adds name="value" to every sample of a text in the Prometheus format; value "":
// the text as it is. value must need no escaping (New checks the stack's name).
func withLabel(text []byte, name, value string) []byte {
	if value == "" {
		return text
	}
	label := name + `="` + value + `"`
	out := make([]byte, 0, len(text)+len(text)/8)
	for len(text) > 0 {
		line := text
		rest := []byte(nil)
		if i := bytes.IndexByte(text, '\n'); i >= 0 {
			line, rest = text[:i+1], text[i+1:]
		}
		text = rest
		if len(line) == 0 || line[0] == '#' || line[0] == '\n' {
			out = append(out, line...)
			continue
		}
		cut := bytes.IndexAny(line, "{ ")
		switch {
		case cut < 0:
			out = append(out, line...)
		case line[cut] == '{':
			out = append(out, line[:cut+1]...)
			out = append(out, label...)
			if line[cut+1] != '}' {
				out = append(out, ',')
			}
			out = append(out, line[cut+1:]...)
		default:
			out = append(out, line[:cut]...)
			out = append(out, '{')
			out = append(out, label...)
			out = append(out, '}')
			out = append(out, line[cut:]...)
		}
	}
	return out
}
