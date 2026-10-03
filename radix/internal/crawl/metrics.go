package crawl

import (
	"strconv"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
)

// What the crawl sends to the university, directly or through Cortex, by source
// (catalogdb.Source*): every request, retries included, and what became of each page.
var (
	requestsTotal = metrics.Default.NewCounter("radix_crawl_requests_total",
		"HTTP requests sent to the university (or to Cortex in its place), retries included, by source and answer (the university's status code, or \"error\" without one, as for an error of Cortex's own).",
		"source", "code")
	requestSeconds = metrics.Default.NewHistogram("radix_crawl_request_duration_seconds",
		"Time from request to the end of the body.",
		[]float64{0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60}, "source")
	responseBytes = metrics.Default.NewCounter("radix_crawl_response_bytes_total",
		"Bytes of response bodies received.", "source")
	pagesTotal = metrics.Default.NewCounter("radix_crawl_pages_total",
		"Pages fetched and archived, by outcome: changed (new or another body than the archived one), unchanged, not_found (404) or failed (given up after retries).",
		"source", "outcome")
)

// The series that are there from the start, at zero: increase() cannot see the first count of
// a series that appears in the middle of a time range (a first 500, a first failed page).
func init() {
	for _, source := range []string{catalogdb.SourceModuleCatalog, catalogdb.SourceModulePage, catalogdb.SourceQISModuleList,
		catalogdb.SourceQISFUESList, catalogdb.SourceQISModulePage, catalogdb.SourceQISTree, catalogdb.SourceQISEventEntry,
		catalogdb.SourceQISEvent} {
		for _, code := range []string{"200", "404", "500", "502", "503", "504", "error"} {
			requestsTotal.Add(0, source, code)
		}
		for _, outcome := range []string{"changed", "unchanged", "not_found", "failed"} {
			pagesTotal.Add(0, source, outcome)
		}
	}
}

func countRequest(source string, status int, err error, took time.Duration, bytes int) {
	code := "error"
	if err == nil || status != 0 {
		code = strconv.Itoa(status)
	}
	requestsTotal.Inc(source, code)
	requestSeconds.Observe(took.Seconds(), source)
	if bytes > 0 {
		responseBytes.Add(float64(bytes), source)
	}
}

// CountPage counts the outcome of a page that was archived from a larger answer (the
// entries of the QIS event search): "changed", "unchanged" or "not_found".
func CountPage(source, outcome string) { pagesTotal.Inc(source, outcome) }

func changedOutcome(changed bool) string {
	if changed {
		return "changed"
	}
	return "unchanged"
}
