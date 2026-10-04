// Package metrics keeps Radix's counters and gauges and writes them in the Prometheus text
// format (GET /metrics). It is the handful of types Radix needs, not a client library:
// counters, gauges and histograms with labels, and gauges read when they are scraped.
//
// Every metric lives in Default, so a package declares its own next to the code it counts.
// Label values must come from a small, fixed set (a source, a stage, a status code): every
// combination is a series of its own, forever.
package metrics

import (
	"bufio"
	"fmt"
	"io"
	"math"
	"net/http"
	"sort"
	"strconv"
	"strings"
	"sync"
)

// Registry holds metric families in the order they were declared.
type Registry struct {
	mu       sync.Mutex
	families []*family
	names    map[string]bool
}

// NewRegistry returns an empty registry. Tests use their own; everything else uses Default.
func NewRegistry() *Registry { return &Registry{names: make(map[string]bool)} }

// Default is the registry GET /metrics serves.
var Default = NewRegistry()

type family struct {
	name, help, typ string
	labels          []string
	buckets         []float64 // histograms

	mu     sync.Mutex
	series map[string]*series
	read   func(emit func(value float64, labelValues ...string)) // gauges read at scrape time
}

type series struct {
	values  []string
	value   float64
	counts  []uint64 // histograms: per bucket, not cumulative
	sum     float64
	samples uint64
}

func (r *Registry) add(f *family) *family {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.names[f.name] {
		panic("metrics: " + f.name + " declared twice")
	}
	r.names[f.name] = true
	f.series = make(map[string]*series)
	r.families = append(r.families, f)
	return f
}

func (f *family) get(labelValues []string) *series {
	if len(labelValues) != len(f.labels) {
		panic(fmt.Sprintf("metrics: %s takes %d label values, got %d", f.name, len(f.labels), len(labelValues)))
	}
	key := strings.Join(labelValues, "\xff")
	s := f.series[key]
	if s == nil {
		s = &series{values: append([]string(nil), labelValues...)}
		if f.buckets != nil {
			s.counts = make([]uint64, len(f.buckets))
		}
		f.series[key] = s
	}
	return s
}

// Counter only goes up; Prometheus notices a restart by its fall to zero.
type Counter struct{ f *family }

// NewCounter declares a counter in r.
func (r *Registry) NewCounter(name, help string, labels ...string) *Counter {
	return &Counter{r.add(&family{name: name, help: help, typ: "counter", labels: labels})}
}

// Add adds v (>= 0) to the series of labelValues.
func (c *Counter) Add(v float64, labelValues ...string) {
	if v < 0 {
		panic("metrics: " + c.f.name + " cannot go down")
	}
	c.f.mu.Lock()
	c.f.get(labelValues).value += v
	c.f.mu.Unlock()
}

// Inc adds one.
func (c *Counter) Inc(labelValues ...string) { c.Add(1, labelValues...) }

// Gauge is a value that is set.
type Gauge struct{ f *family }

// NewGauge declares a gauge in r.
func (r *Registry) NewGauge(name, help string, labels ...string) *Gauge {
	return &Gauge{r.add(&family{name: name, help: help, typ: "gauge", labels: labels})}
}

// Set sets the series of labelValues to v.
func (g *Gauge) Set(v float64, labelValues ...string) {
	g.f.mu.Lock()
	g.f.get(labelValues).value = v
	g.f.mu.Unlock()
}

// NewGaugeFunc declares a gauge whose series read reports at every scrape (emit once per
// series); nothing is kept between scrapes.
func (r *Registry) NewGaugeFunc(name, help string, labels []string, read func(emit func(value float64, labelValues ...string))) {
	r.add(&family{name: name, help: help, typ: "gauge", labels: labels, read: read})
}

// Histogram counts observations into buckets, for durations and sizes.
type Histogram struct{ f *family }

// NewHistogram declares a histogram in r with the given upper bounds (ascending; +Inf is implied).
func (r *Registry) NewHistogram(name, help string, buckets []float64, labels ...string) *Histogram {
	if !sort.Float64sAreSorted(buckets) {
		panic("metrics: " + name + ": buckets must ascend")
	}
	return &Histogram{r.add(&family{name: name, help: help, typ: "histogram", labels: labels, buckets: buckets})}
}

// Observe records v in the series of labelValues.
func (h *Histogram) Observe(v float64, labelValues ...string) {
	h.f.mu.Lock()
	s := h.f.get(labelValues)
	if i := sort.SearchFloat64s(h.f.buckets, v); i < len(h.f.buckets) {
		s.counts[i]++
	}
	s.sum += v
	s.samples++
	h.f.mu.Unlock()
}

// WriteText writes every family in the Prometheus text format, version 0.0.4.
func (r *Registry) WriteText(w io.Writer) error {
	r.mu.Lock()
	families := append([]*family(nil), r.families...)
	r.mu.Unlock()

	bw := bufio.NewWriter(w)
	for _, f := range families {
		f.write(bw)
	}
	return bw.Flush()
}

func (f *family) write(w *bufio.Writer) {
	var list []*series
	if f.read != nil {
		f.read(func(v float64, labelValues ...string) {
			if len(labelValues) != len(f.labels) {
				panic(fmt.Sprintf("metrics: %s takes %d label values, got %d", f.name, len(f.labels), len(labelValues)))
			}
			list = append(list, &series{values: append([]string(nil), labelValues...), value: v})
		})
	} else {
		f.mu.Lock()
		for _, s := range f.series {
			c := *s
			c.values = append([]string(nil), s.values...)
			c.counts = append([]uint64(nil), s.counts...)
			list = append(list, &c)
		}
		f.mu.Unlock()
	}
	if len(list) == 0 {
		return
	}
	sort.Slice(list, func(i, j int) bool {
		return strings.Join(list[i].values, "\xff") < strings.Join(list[j].values, "\xff")
	})

	fmt.Fprintf(w, "# HELP %s %s\n# TYPE %s %s\n", f.name, escapeHelp(f.help), f.name, f.typ)
	for _, s := range list {
		if f.buckets == nil {
			fmt.Fprintf(w, "%s%s %s\n", f.name, f.labelSet(s.values, "", ""), number(s.value))
			continue
		}
		var cumulative uint64
		for i, bound := range f.buckets {
			cumulative += s.counts[i]
			fmt.Fprintf(w, "%s_bucket%s %d\n", f.name, f.labelSet(s.values, "le", number(bound)), cumulative)
		}
		fmt.Fprintf(w, "%s_bucket%s %d\n", f.name, f.labelSet(s.values, "le", "+Inf"), s.samples)
		fmt.Fprintf(w, "%s_sum%s %s\n", f.name, f.labelSet(s.values, "", ""), number(s.sum))
		fmt.Fprintf(w, "%s_count%s %d\n", f.name, f.labelSet(s.values, "", ""), s.samples)
	}
}

func (f *family) labelSet(values []string, extraName, extraValue string) string {
	if len(values) == 0 && extraName == "" {
		return ""
	}
	var b strings.Builder
	b.WriteByte('{')
	for i, name := range f.labels {
		if i > 0 {
			b.WriteByte(',')
		}
		fmt.Fprintf(&b, "%s=\"%s\"", name, escapeValue(values[i]))
	}
	if extraName != "" {
		if len(values) > 0 {
			b.WriteByte(',')
		}
		fmt.Fprintf(&b, "%s=\"%s\"", extraName, extraValue)
	}
	b.WriteByte('}')
	return b.String()
}

func number(v float64) string {
	switch {
	case math.IsInf(v, 1):
		return "+Inf"
	case math.IsInf(v, -1):
		return "-Inf"
	case math.IsNaN(v):
		return "NaN"
	}
	return strconv.FormatFloat(v, 'g', -1, 64)
}

var (
	helpEscaper  = strings.NewReplacer(`\`, `\\`, "\n", `\n`)
	valueEscaper = strings.NewReplacer(`\`, `\\`, "\n", `\n`, `"`, `\"`)
)

func escapeHelp(s string) string  { return helpEscaper.Replace(s) }
func escapeValue(s string) string { return valueEscaper.Replace(s) }

// Handler serves r in the Prometheus text format.
func (r *Registry) Handler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
		w.Header().Set("Cache-Control", "no-store")
		_ = r.WriteText(w)
	})
}

// Bool is 1 for true and 0 for false, for gauges that say yes or no.
func Bool(b bool) float64 {
	if b {
		return 1
	}
	return 0
}
