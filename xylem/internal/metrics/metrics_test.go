package metrics

import (
	"net/http/httptest"
	"strings"
	"testing"
)

func TestWriteText(t *testing.T) {
	r := NewRegistry()
	requests := r.NewCounter("xylem_test_requests_total", "Requests.\nSecond line.", "source", "code")
	requests.Inc("qis_event", "200")
	requests.Add(2, "module_page", "200")
	requests.Inc("module_page", "200")
	requests.Inc("qis_event", `say "no"`)
	r.NewGauge("xylem_test_empty", "Never set.")
	healthy := r.NewGauge("xylem_test_healthy", "Health.")
	healthy.Set(Bool(true))
	latency := r.NewHistogram("xylem_test_seconds", "Latency.", []float64{0.1, 1}, "source")
	latency.Observe(0.05, "qis_event")
	latency.Observe(0.1, "qis_event")
	latency.Observe(0.5, "qis_event")
	latency.Observe(3, "qis_event")
	r.NewGaugeFunc("xylem_test_pages", "Pages.", []string{"source"}, func(emit func(float64, ...string)) {
		emit(7, "qis_tree")
		emit(3, "module_page")
	})

	rec := httptest.NewRecorder()
	r.Handler().ServeHTTP(rec, httptest.NewRequest("GET", "/metrics", nil))
	want := `# HELP xylem_test_requests_total Requests.\nSecond line.
# TYPE xylem_test_requests_total counter
xylem_test_requests_total{source="module_page",code="200"} 3
xylem_test_requests_total{source="qis_event",code="200"} 1
xylem_test_requests_total{source="qis_event",code="say \"no\""} 1
# HELP xylem_test_healthy Health.
# TYPE xylem_test_healthy gauge
xylem_test_healthy 1
# HELP xylem_test_seconds Latency.
# TYPE xylem_test_seconds histogram
xylem_test_seconds_bucket{source="qis_event",le="0.1"} 2
xylem_test_seconds_bucket{source="qis_event",le="1"} 3
xylem_test_seconds_bucket{source="qis_event",le="+Inf"} 4
xylem_test_seconds_sum{source="qis_event"} 3.65
xylem_test_seconds_count{source="qis_event"} 4
# HELP xylem_test_pages Pages.
# TYPE xylem_test_pages gauge
xylem_test_pages{source="module_page"} 3
xylem_test_pages{source="qis_tree"} 7
`
	if got := rec.Body.String(); got != want {
		t.Errorf("got\n%s\nwant\n%s", got, want)
	}
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "text/plain; version=0.0.4") {
		t.Errorf("Content-Type %q", ct)
	}
}

func TestDeclaredTwice(t *testing.T) {
	r := NewRegistry()
	r.NewCounter("xylem_twice_total", "")
	defer func() {
		if recover() == nil {
			t.Error("a second declaration of the same name did not panic")
		}
	}()
	r.NewGauge("xylem_twice_total", "")
}
