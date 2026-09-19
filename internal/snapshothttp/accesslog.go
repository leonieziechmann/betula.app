package snapshothttp

import (
	"log/slog"
	"net/http"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/oplog"
)

type statusWriter struct {
	http.ResponseWriter
	status int
	bytes  int64
}

func (w *statusWriter) WriteHeader(status int) {
	w.status = status
	w.ResponseWriter.WriteHeader(status)
}

func (w *statusWriter) Write(b []byte) (int, error) {
	n, err := w.ResponseWriter.Write(b)
	w.bytes += int64(n)
	return n, err
}

// AccessLog logs every request (event http.request). Health checks and 304
// answers are DEBUG, because they arrive every few seconds and say nothing new;
// a 5xx answer is an ERROR, any other 4xx/503 a WARN.
func AccessLog(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		sw := &statusWriter{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(sw, r)

		level := slog.LevelInfo
		switch {
		case sw.status == http.StatusServiceUnavailable || (sw.status >= 400 && sw.status < 500):
			level = slog.LevelWarn
		case sw.status >= 500:
			level = slog.LevelError
		case sw.status == http.StatusNotModified || r.URL.Path == "/healthz":
			level = slog.LevelDebug
		}
		oplog.For("http").Log(r.Context(), level, "request", "event", "http.request", "method", r.Method, "path", r.URL.Path,
			"status", sw.status, "bytes", sw.bytes, "duration_ms", time.Since(start).Milliseconds(),
			"remote", r.RemoteAddr, "user_agent", r.UserAgent())
	})
}
