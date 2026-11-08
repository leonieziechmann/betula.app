package server

import (
	"bytes"
	"os"
	"path/filepath"
	"time"

	"github.com/leonieziechmann/betula/phloem/internal/metrics"
)

// StatsFile is the name of the stats file in Options.StatsDir. Its temporary twin does not end
// in .prom, so that a reader of *.prom never sees half a file.
const (
	StatsFile = "phloem.prom"
	statsTemp = ".phloem.prom.tmp"
)

// meters are the counters of the stats file; the gauges read the server when it is written.
// Label values come from fixed sets (docs/phloem/phloem.md, „Numbers").
type meters struct {
	entries    *metrics.Counter // pushes kept
	entryBytes *metrics.Counter
	again      *metrics.Counter // pushes sent again, answered with their entry
	snapshots  *metrics.Counter
	snapBytes  *metrics.Counter
	rejected   *metrics.Counter // by reason: a code of the protocol, or origin, busy, invalid
	created    *metrics.Counter // spaces made
	deleted    *metrics.Counter // spaces deleted, by reason: deleted, expired
	parcels    *metrics.Counter // by event: left, taken, missed, expired, withdrawn
	parts      *metrics.Counter // by event: put, deleted, served, not_modified, missed, gone
	closes     *metrics.Counter // sockets closed by Phloem or lost, by reason

	signals      *metrics.Counter // signals handed on (step 2)
	turn         *metrics.Counter // TURN, by event
	relayed      *metrics.Counter // bytes the relays passed, by way (in: from peers, out: to them)
	relayDropped *metrics.Counter // bytes the relays dropped for the rates, by way
}

func (s *Server) declare() {
	r := s.reg
	s.m = meters{
		entries:    r.NewCounter("phloem_entries_total", "Pushes kept as entries."),
		entryBytes: r.NewCounter("phloem_entry_bytes_total", "Sealed bytes of the pushes kept."),
		again:      r.NewCounter("phloem_pushes_again_total", "Pushes a device sent again, answered with the entry they had."),
		snapshots:  r.NewCounter("phloem_snapshots_total", "Snapshots kept."),
		snapBytes:  r.NewCounter("phloem_snapshot_bytes_total", "Sealed bytes of the snapshots kept."),
		rejected: r.NewCounter("phloem_rejections_total",
			"Requests and frames refused, by reason (unauthorized, too_big, compact_first, slow_down, too_many, full, budget, exists, origin, busy, invalid).", "reason"),
		created: r.NewCounter("phloem_spaces_created_total", "Spaces made."),
		deleted: r.NewCounter("phloem_spaces_deleted_total", "Spaces deleted, by reason (deleted: Sync beenden; expired: untouched for 395 days).", "reason"),
		parcels: r.NewCounter("phloem_parcels_total", "Parcels for devices to come, by event (left, taken, missed, expired, withdrawn).", "event"),
		parts: r.NewCounter("phloem_parts_total",
			"Parts of shared calendars, by event (put: made or replaced; deleted: by Nicht mehr teilen or with their space; served, not_modified, missed, gone: reads answered 200, 304, 404, 410).", "event"),
		closes: r.NewCounter("phloem_socket_closes_total",
			"Sockets that ended, by reason (left, gone, replaced, restart, slow, timeout, lost, invalid, internal).", "reason"),
		signals: r.NewCounter("phloem_signals_total", "Signals for WebRTC handed from one device to another of its space."),
		turn: r.NewCounter("phloem_turn_total",
			"TURN, by event (allocated, permitted; refused: a peer that is not another device of the space; unauthorized; quota; no_port; forbidden: a relay was to send where no peer may be, which must never happen).", "event"),
		relayed:      r.NewCounter("phloem_relay_bytes_total", "Bytes the TURN relays passed, by way (in: from a peer, out: to a peer).", "way"),
		relayDropped: r.NewCounter("phloem_relay_dropped_bytes_total", "Bytes the TURN relays dropped beyond their rates, by way.", "way"),
	}
	r.NewGaugeFunc("phloem_up", "1 while Phloem runs; the last file it writes says 0.", nil,
		func(emit func(float64, ...string)) { emit(metrics.Bool(s.up.Load())) })
	build := s.opt.Build
	if len(build) > 12 {
		build = build[:12]
	}
	r.NewGaugeFunc("phloem_build_info", "Always 1: the version and build (the start of the sha256 of the executable) that run.",
		[]string{"version", "build"}, func(emit func(float64, ...string)) { emit(1, s.opt.Version, build) })
	started := float64(s.started.Unix())
	r.NewGaugeFunc("phloem_start_time_seconds", "When the process started (Unix time).", nil,
		func(emit func(float64, ...string)) { emit(started) })
	r.NewGaugeFunc("phloem_spaces", "Spaces kept.", nil, func(emit func(float64, ...string)) {
		n, _ := s.st.Totals()
		emit(float64(n))
	})
	r.NewGaugeFunc("phloem_bytes", "Bytes all spaces hold, for the budget (entries, snapshots, parcels, parts, rows; a tombstone is a row).", nil,
		func(emit func(float64, ...string)) {
			_, b := s.st.Totals()
			emit(float64(b))
		})
	r.NewGaugeFunc("phloem_budget_bytes", "The budget: at it, no new spaces, parcels or parts.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.budget)) })
	r.NewGaugeFunc("phloem_parts", "Parts of shared calendars kept.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.parts.Load())) })
	r.NewGaugeFunc("phloem_parts_watched", "Parts that sockets watch.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.watchers.size())) })
	r.NewGaugeFunc("phloem_devices", "Devices with a slot in a space.", nil,
		func(emit func(float64, ...string)) { emit(float64(s.devices.Load())) })
	r.NewGaugeFunc("phloem_sockets", "Open sockets.", nil, func(emit func(float64, ...string)) {
		s.mu.Lock()
		n := s.conns
		s.mu.Unlock()
		emit(float64(n))
	})
	r.NewGaugeFunc("phloem_spaces_online", "Spaces with a device online.", nil, func(emit func(float64, ...string)) {
		s.mu.Lock()
		n := len(s.rooms)
		s.mu.Unlock()
		emit(float64(n))
	})
	r.NewGaugeFunc("phloem_turn_allocations", "TURN allocations now.", nil, func(emit func(float64, ...string)) {
		if s.turn == nil {
			emit(0)
			return
		}
		s.turn.mu.Lock()
		n := s.turn.allocs
		s.turn.mu.Unlock()
		emit(float64(n))
	})
	r.NewGaugeFunc("phloem_stats_time_seconds", "When this file was written (Unix time).", nil,
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
