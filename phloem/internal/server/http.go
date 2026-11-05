package server

import (
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

// The sizes of the values of the protocol, in bytes (docs/phloem/protocol.md section 3).
const (
	spaceLen  = 16
	tokenLen  = 32
	deviceLen = 16
	pairLen   = 16
)

// bodyTimeout bounds the reading of a request body (the server only bounds the headers, since a
// read deadline would also end the WebSockets).
const bodyTimeout = 30 * time.Second

// Handler is Phloem's HTTP API: /sync/v1/ (Traefik routes it), /healthz and /status (it does
// not; /status answers loopback only).
func (s *Server) Handler() http.Handler { return http.HandlerFunc(s.serveHTTP) }

func (s *Server) serveHTTP(w http.ResponseWriter, r *http.Request) {
	h := w.Header()
	h.Set("Cache-Control", "no-store")
	h.Set("X-Content-Type-Options", "nosniff")
	if strings.HasPrefix(r.URL.Path, "/sync/") {
		h.Add("Vary", "Origin")
		if origin := r.Header.Get("Origin"); origin != "" {
			if !s.originAllowed(origin, r.Host) {
				s.fail(w, http.StatusForbidden, "origin")
				return
			}
			h.Set("Access-Control-Allow-Origin", origin)
		}
		if r.Method == http.MethodOptions {
			h.Set("Access-Control-Allow-Methods", "GET, POST, DELETE")
			h.Set("Access-Control-Allow-Headers", "Authorization, Betula-Space, Betula-Pair, Content-Type")
			h.Set("Access-Control-Max-Age", "600")
			w.WriteHeader(http.StatusNoContent)
			return
		}
	}
	switch path, method := r.URL.Path, r.Method; {
	case path == "/sync/v1/space" && method == http.MethodPost:
		s.createSpace(w, r)
	case path == "/sync/v1/space" && method == http.MethodDelete:
		s.removeSpace(w, r)
	case path == "/sync/v1/pair" && method == http.MethodPost:
		s.leavePair(w, r)
	case path == "/sync/v1/pair" && method == http.MethodGet:
		s.takePair(w, r)
	case path == "/sync/v1/ws" && method == http.MethodGet:
		s.serveWS(w, r)
	case path == "/healthz" && (method == http.MethodGet || method == http.MethodHead):
		s.healthz(w, r)
	case path == "/status" && method == http.MethodGet && fromLoopback(r):
		s.status(w, r)
	case path == "/sync/v1/space", path == "/sync/v1/pair", path == "/sync/v1/ws", path == "/healthz":
		h.Set("Allow", allowed(path))
		writeError(w, http.StatusMethodNotAllowed, "method")
	default:
		writeError(w, http.StatusNotFound, "not_found")
	}
}

func allowed(path string) string {
	switch path {
	case "/sync/v1/space":
		return "POST, DELETE, OPTIONS"
	case "/sync/v1/pair":
		return "GET, POST, OPTIONS"
	case "/healthz":
		return "GET, HEAD"
	}
	return "GET"
}

// originAllowed says whether a browser of origin may use Phloem: one of Options.Origins, or
// without them the origin of the request's own host.
func (s *Server) originAllowed(origin, host string) bool {
	origin = strings.ToLower(origin)
	if len(s.origins) > 0 {
		return s.origins[origin]
	}
	u, err := url.Parse(origin)
	if err != nil || u.Host == "" || (u.Scheme != "https" && u.Scheme != "http") {
		return false
	}
	return strings.EqualFold(u.Host, host)
}

// fail answers an error of the protocol and counts it.
func (s *Server) fail(w http.ResponseWriter, status int, code string) {
	s.m.rejected.Inc(code)
	writeError(w, status, code)
}

func writeError(w http.ResponseWriter, status int, code string) {
	writeJSON(w, status, map[string]string{"error": code})
}

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

// internal answers 500 for a store call that failed.
func (s *Server) internal(w http.ResponseWriter, op string, err error) {
	s.storeFailed(op, err)
	writeError(w, http.StatusInternalServerError, "internal")
}

// readBody reads a JSON body of at most limit bytes into v; false after it answered.
func (s *Server) readBody(w http.ResponseWriter, r *http.Request, limit int64, v any) bool {
	_ = http.NewResponseController(w).SetReadDeadline(time.Now().Add(bodyTimeout))
	data, err := io.ReadAll(http.MaxBytesReader(w, r.Body, limit))
	if err != nil {
		var tooBig *http.MaxBytesError
		if errors.As(err, &tooBig) {
			s.fail(w, http.StatusRequestEntityTooLarge, "too_big")
		} else {
			s.fail(w, http.StatusBadRequest, "invalid")
		}
		return false
	}
	if err := json.Unmarshal(data, v); err != nil {
		s.fail(w, http.StatusBadRequest, "invalid")
		return false
	}
	return true
}

// clientKey is what the limits of an address count by: Traefik's X-Real-Ip (it trusts no
// forwarded header, so it is the address the request came from), else the peer's address; an
// IPv6 address by its /64, which one end of a line usually holds whole.
func clientKey(r *http.Request) string {
	raw := strings.TrimSpace(r.Header.Get("X-Real-Ip"))
	if raw == "" {
		host, _, err := net.SplitHostPort(r.RemoteAddr)
		if err != nil {
			host = r.RemoteAddr
		}
		raw = host
	}
	ip, err := netip.ParseAddr(raw)
	if err != nil {
		return "?"
	}
	ip = ip.Unmap().WithZone("")
	if ip.Is6() {
		if p, err := ip.Prefix(64); err == nil {
			return p.String()
		}
	}
	return ip.String()
}

// fromLoopback says whether a request comes from inside the container: a loopback peer, and no
// header a proxy would have added.
func fromLoopback(r *http.Request) bool {
	if r.Header.Get("X-Real-Ip") != "" || r.Header.Get("X-Forwarded-For") != "" {
		return false
	}
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return false
	}
	ip, err := netip.ParseAddr(host)
	return err == nil && ip.IsLoopback()
}

// authenticate checks the headers of a space (Betula-Space, Authorization: Bearer); false after
// it answered 401.
func (s *Server) authenticate(w http.ResponseWriter, r *http.Request) (store.Space, bool) {
	id, ok := decode64(r.Header.Get("Betula-Space"), spaceLen)
	bearer, found := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
	token, ok2 := decode64(strings.TrimSpace(bearer), tokenLen)
	if !ok || !found || !ok2 {
		s.fail(w, http.StatusUnauthorized, "unauthorized")
		return store.Space{}, false
	}
	sp, err := s.st.Authenticate(r.Context(), id, token)
	if errors.Is(err, store.ErrUnauthorized) {
		s.fail(w, http.StatusUnauthorized, "unauthorized")
		return store.Space{}, false
	}
	if err != nil {
		s.internal(w, "authenticate", err)
		return store.Space{}, false
	}
	return sp, true
}

// createSpace is POST /sync/v1/space: a space with the hash of its token, its first device in
// slot 1. 409 exists, 429 slow_down (per address and in all), 507 budget.
func (s *Server) createSpace(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Space  b64 `json:"space"`
		Token  b64 `json:"token"`
		Device b64 `json:"device"`
	}
	if !s.readBody(w, r, 1<<10, &body) {
		return
	}
	if len(body.Space) != spaceLen || len(body.Token) != tokenLen || len(body.Device) != deviceLen {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	if s.overBudget() {
		s.fail(w, http.StatusInsufficientStorage, "budget")
		return
	}
	key, now := clientKey(r), s.now()
	if !s.creates.take(key, 1, now) {
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	if !s.createAll.take("", 1, now) {
		s.creates.give(key, 1)
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	err := s.st.CreateSpace(r.Context(), body.Space, store.TokenHash(body.Token), body.Device, now)
	if errors.Is(err, store.ErrExists) {
		// Nothing was made (often the same device again, whose answer was lost).
		s.creates.give(key, 1)
		s.createAll.give("", 1)
		s.fail(w, http.StatusConflict, "exists")
		return
	}
	if err != nil {
		s.creates.give(key, 1)
		s.createAll.give("", 1)
		s.internal(w, "create", err)
		return
	}
	s.m.created.Inc()
	writeJSON(w, http.StatusCreated, map[string]int{"slot": 1})
}

// removeSpace is DELETE /sync/v1/space („Sync beenden"): the space and everything of it; its
// devices online hear gone. 204.
func (s *Server) removeSpace(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	if _, err := s.deleteSpace(sp.ID, "deleted"); err != nil {
		s.internal(w, "delete", err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// leavePair is POST /sync/v1/pair: a sealed parcel for a device to come, kept Limits.PairTTL.
// 201; 409 exists; 413 too_big; 429 slow_down (the space's pushes, or as many parcels waiting
// as it may have); 507 budget.
func (s *Server) leavePair(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	var body struct {
		ID  b64 `json:"id"`
		Box b64 `json:"box"`
	}
	if !s.readBody(w, r, int64(b64len(s.lim.MaxBox))+1<<10, &body) {
		return
	}
	if len(body.ID) != pairLen || body.Box == nil {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	if len(body.Box) > s.lim.MaxBox {
		s.fail(w, http.StatusRequestEntityTooLarge, "too_big")
		return
	}
	if s.overBudget() {
		s.fail(w, http.StatusInsufficientStorage, "budget")
		return
	}
	now := s.now()
	if !s.pushes.take(string(sp.ID), 1, now) {
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	err := s.st.PutPair(r.Context(), body.ID, sp.ID, body.Box, now, now.Add(s.lim.PairTTL), s.lim.MaxPairs)
	switch {
	case errors.Is(err, store.ErrExists):
		s.fail(w, http.StatusConflict, "exists")
	case errors.Is(err, store.ErrTooMany):
		s.fail(w, http.StatusTooManyRequests, "slow_down")
	case errors.Is(err, store.ErrNotFound):
		s.fail(w, http.StatusUnauthorized, "unauthorized") // deleted meanwhile
	case err != nil:
		s.internal(w, "pair", err)
	default:
		s.m.parcels.Inc("left")
		writeJSON(w, http.StatusCreated, struct{}{})
	}
}

// takePair is GET /sync/v1/pair with Betula-Pair: the parcel once. 200 {"box"}; 404 not_found;
// 429 slow_down (Limits.TakesPerAddr).
func (s *Server) takePair(w http.ResponseWriter, r *http.Request) {
	id, ok := decode64(r.Header.Get("Betula-Pair"), pairLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	now := s.now()
	if !s.takes.take(clientKey(r), 1, now) {
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	box, err := s.st.TakePair(r.Context(), id, now)
	if errors.Is(err, store.ErrNotFound) {
		s.m.parcels.Inc("missed")
		writeError(w, http.StatusNotFound, "not_found")
		return
	}
	if err != nil {
		s.internal(w, "take", err)
		return
	}
	s.m.parcels.Inc("taken")
	writeJSON(w, http.StatusOK, struct {
		Box b64 `json:"box"`
	}{box})
}

// healthz is GET /healthz: 200 ok while the store answers.
func (s *Server) healthz(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	if err := s.st.Ping(r.Context()); err != nil {
		w.WriteHeader(http.StatusServiceUnavailable)
		_, _ = io.WriteString(w, "unavailable\n")
		return
	}
	_, _ = io.WriteString(w, "ok\n")
}

// Status is what GET /status (loopback only) and `phloem status` say.
type Status struct {
	Version      string    `json:"version"`
	Build        string    `json:"build"`
	Started      time.Time `json:"started"`
	Spaces       int64     `json:"spaces"`
	Bytes        int64     `json:"bytes"`
	Budget       int64     `json:"budget"`
	BudgetUsed   float64   `json:"budget_used"`
	Devices      int64     `json:"devices"`
	Sockets      int       `json:"sockets"`
	SpacesOnline int       `json:"spaces_online"`
	Stats        string    `json:"stats_dir"`
}

// Status is the server's status now.
func (s *Server) Status() Status {
	spaces, bytes := s.st.Totals()
	s.mu.Lock()
	conns, online := s.conns, len(s.rooms)
	s.mu.Unlock()
	return Status{
		Version: s.opt.Version, Build: s.opt.Build, Started: s.started.UTC().Truncate(time.Second),
		Spaces: spaces, Bytes: bytes, Budget: s.budget, BudgetUsed: float64(bytes) / float64(s.budget),
		Devices: s.devices.Load(), Sockets: conns, SpacesOnline: online, Stats: s.opt.StatsDir,
	}
}

func (s *Server) status(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, s.Status())
}
