package web

import (
	"context"
	"embed"
	"fmt"
	"html/template"
	"io/fs"
	"net/http"
	"strings"
	"time"

	"github.com/jakob/btu-scraper/internal/analytics"
	"github.com/jakob/btu-scraper/internal/logger"
	"github.com/jakob/btu-scraper/internal/provider"
	"github.com/jakob/btu-scraper/internal/refresher"
	"github.com/jakob/btu-scraper/internal/storage"
)

//go:embed templates/* static/*
var contentFS embed.FS

// Server represents the HTMX web application server.
type Server struct {
	store     *storage.Storage
	eventProv *provider.BTUEventProvider
	tracker   *analytics.Tracker
	refresher *refresher.Refresher
	logger    *logger.Logger
	startTime time.Time
	templates *template.Template
	mux       *http.ServeMux
	server    *http.Server
}

// ServerOption configures optional services for Server.
type ServerOption func(*Server)

// WithTracker configures the anonymous analytics tracker.
func WithTracker(t *analytics.Tracker) ServerOption {
	return func(s *Server) { s.tracker = t }
}

// WithRefresher configures the background polite crawler refresher.
func WithRefresher(r *refresher.Refresher) ServerOption {
	return func(s *Server) { s.refresher = r }
}

// WithLogger configures the system logger.
func WithLogger(l *logger.Logger) ServerOption {
	return func(s *Server) { s.logger = l }
}

// NewServer initializes the web server and parses templates.
func NewServer(store *storage.Storage, eventProv *provider.BTUEventProvider, opts ...ServerOption) (*Server, error) {
	tmplFuncs := template.FuncMap{
		"stringsJoin":     strings.Join,
		"stringsContains": strings.Contains,
		"hasPrefix":       strings.HasPrefix,
		"safeJS": func(s string) template.JS {
			return template.JS(s)
		},
		"slice": func(args ...int) []int {
			return args
		},
		"truncate": func(s string, max int) string {
			if len(s) > max {
				return s[:max-3] + "..."
			}
			return s
		},
		"formatCredits": func(raw string, c float64) string {
			if raw != "" {
				r := strings.TrimSpace(raw)
				if strings.HasSuffix(r, ".0") {
					return strings.TrimSuffix(r, ".0")
				}
				return r
			}
			if c == float64(int(c)) {
				return fmt.Sprintf("%d", int(c))
			}
			return fmt.Sprintf("%.1f", c)
		},
		"formatTurnusShort": func(t string) string {
			low := strings.ToLower(strings.TrimSpace(t))
			if low == "" {
				return "k. A."
			}
			if strings.Contains(low, "jedes semester") || strings.Contains(low, "every semester") {
				return "🔄 Jedes Sem."
			}
			if strings.Contains(low, "winter") {
				if strings.Contains(low, "gerad") || strings.Contains(low, "even") {
					return "❄️ WiSe (ger.)"
				}
				if strings.Contains(low, "ungerad") || strings.Contains(low, "odd") {
					return "❄️ WiSe (ung.)"
				}
				return "❄️ WiSe"
			}
			if strings.Contains(low, "sommer") || strings.Contains(low, "summer") {
				if strings.Contains(low, "gerad") || strings.Contains(low, "even") {
					return "☀️ SoSe (ger.)"
				}
				if strings.Contains(low, "ungerad") || strings.Contains(low, "odd") {
					return "☀️ SoSe (ung.)"
				}
				return "☀️ SoSe"
			}
			if strings.Contains(low, "sporadisch") || strings.Contains(low, "ankündigung") || strings.Contains(low, "announcement") {
				return "🎲 Sporadisch"
			}
			return t
		},
		"formatLangBadge": func(l string) string {
			low := strings.ToLower(strings.TrimSpace(l))
			if strings.Contains(low, "deutsch") && strings.Contains(low, "engl") {
				return "🇩🇪/🇬🇧"
			}
			if strings.Contains(low, "engl") {
				return "🇬🇧"
			}
			if strings.Contains(low, "deutsch") {
				return "🇩🇪"
			}
			if low == "" {
				return "🇩🇪"
			}
			return l
		},
		"cleanDept": func(d string) string {
			d = strings.TrimSpace(d)
			if idx := strings.LastIndex(d, "/"); idx != -1 && idx < len(d)-1 {
				return strings.TrimSpace(d[idx+1:])
			}
			return d
		},
		"add": func(a, b int) int {
			return a + b
		},
		"sub": func(a, b int) int {
			return a - b
		},
	}

	tmpl, err := template.New("").Funcs(tmplFuncs).ParseFS(contentFS, "templates/*.html")
	if err != nil {
		return nil, fmt.Errorf("failed to parse web templates: %w", err)
	}

	s := &Server{
		store:     store,
		eventProv: eventProv,
		startTime: time.Now(),
		templates: tmpl,
		mux:       http.NewServeMux(),
	}

	for _, opt := range opts {
		opt(s)
	}

	s.routes()
	return s, nil
}

func (s *Server) routes() {
	// Static assets embedded
	staticSubFS, err := fs.Sub(contentFS, "static")
	if err == nil {
		s.mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.FS(staticSubFS))))
	}

	// Main routes
	s.mux.HandleFunc("/", s.wrap(s.handleIndex))
	s.mux.HandleFunc("/modules", s.wrap(s.handleModules))
	s.mux.HandleFunc("/modules/", s.wrap(s.handleModuleModal))
	s.mux.HandleFunc("/stats", s.wrap(s.handleStatsPage))
	s.mux.HandleFunc("/api/programs", s.wrap(s.handleProgramsAPI))
	s.mux.HandleFunc("/api/suggestions", s.wrap(s.handleSuggestionsAPI))
	s.mux.HandleFunc("/api/stats", s.wrap(s.handleStatsAPI))
	s.mux.HandleFunc("/api/track", s.wrap(s.handleTrackAPI))
	s.mux.HandleFunc("/api/logs", s.wrap(s.handleLogsAPI))
}

// wrap provides latency and error rate load monitoring
func (s *Server) wrap(handler http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		rw := &statusResponseWriter{ResponseWriter: w, statusCode: http.StatusOK}
		handler(rw, r)
		duration := time.Since(start)

		if s.logger != nil {
			isErr := rw.statusCode >= 500
			s.logger.RecordHTTPRequest(duration, isErr)
		}
	}
}

type statusResponseWriter struct {
	http.ResponseWriter
	statusCode int
}

func (rw *statusResponseWriter) WriteHeader(code int) {
	rw.statusCode = code
	rw.ResponseWriter.WriteHeader(code)
}


// Start runs the HTTP server listening on the specified port.
func (s *Server) Start(port string) error {
	if !strings.HasPrefix(port, ":") {
		port = ":" + port
	}

	s.server = &http.Server{
		Addr:         port,
		Handler:      s.mux,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	fmt.Printf("\n🚀 BTU Smart Modulkatalog Webserver läuft auf http://localhost%s\n", port)
	fmt.Println("   Drücke Strg+C zum Beenden.")

	return s.server.ListenAndServe()
}

// Shutdown gracefully stops the HTTP server.
func (s *Server) Shutdown(ctx context.Context) error {
	if s.server != nil {
		return s.server.Shutdown(ctx)
	}
	return nil
}
