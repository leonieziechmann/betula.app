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

	"github.com/jakob/btu-scraper/internal/provider"
	"github.com/jakob/btu-scraper/internal/storage"
)

//go:embed templates/* static/*
var contentFS embed.FS

// Server represents the HTMX web application server.
type Server struct {
	store     *storage.Storage
	eventProv *provider.BTUEventProvider
	templates *template.Template
	mux       *http.ServeMux
	server    *http.Server
}

// NewServer initializes the web server and parses templates.
func NewServer(store *storage.Storage, eventProv *provider.BTUEventProvider) (*Server, error) {
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
				return raw
			}
			return fmt.Sprintf("%.1f", c)
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
		templates: tmpl,
		mux:       http.NewServeMux(),
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
	s.mux.HandleFunc("/", s.handleIndex)
	s.mux.HandleFunc("/modules", s.handleModules)
	s.mux.HandleFunc("/modules/", s.handleModuleModal)
	s.mux.HandleFunc("/api/programs", s.handleProgramsAPI)
	s.mux.HandleFunc("/api/suggestions", s.handleSuggestionsAPI)
	s.mux.HandleFunc("/api/stats", s.handleStatsAPI)
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
