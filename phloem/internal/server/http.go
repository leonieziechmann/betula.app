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
	partLen   = 16
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
			h.Set("Access-Control-Expose-Headers", "ETag") // a part's version, for If-None-Match
		}
		if r.Method == http.MethodOptions {
			h.Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE")
			h.Set("Access-Control-Allow-Headers", "Authorization, Betula-Space, Betula-Pair, Betula-Part, Betula-Poll, Content-Type, If-None-Match")
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
	case path == "/sync/v1/pair" && method == http.MethodDelete:
		s.withdrawPair(w, r)
	case path == "/sync/v1/pairs" && method == http.MethodGet:
		s.listPairs(w, r)
	case path == "/sync/v1/part" && method == http.MethodPut:
		s.putPart(w, r)
	case path == "/sync/v1/part" && method == http.MethodDelete:
		s.deletePart(w, r)
	case path == "/sync/v1/part" && method == http.MethodGet:
		s.getPart(w, r)
	case path == "/sync/v1/ws" && method == http.MethodGet:
		s.serveWS(w, r)
	case path == "/sync/v1/poll" && method == http.MethodPost:
		s.postPoll(w, r)
	case path == "/sync/v1/poll" && method == http.MethodGet:
		s.getPoll(w, r)
	case path == "/healthz" && (method == http.MethodGet || method == http.MethodHead):
		s.healthz(w, r)
	case path == "/status" && method == http.MethodGet && fromLoopback(r):
		s.status(w, r)
	case path == "/sync/v1/space", path == "/sync/v1/pair", path == "/sync/v1/pairs", path == "/sync/v1/part",
		path == "/sync/v1/ws", path == "/sync/v1/poll", path == "/healthz":
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
		return "GET, POST, DELETE, OPTIONS"
	case "/sync/v1/pairs":
		return "GET, OPTIONS"
	case "/sync/v1/part":
		return "GET, PUT, DELETE, OPTIONS"
	case "/sync/v1/poll":
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
	ip, err := netip.ParseAddr(clientIP(r))
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

// clientAddr is the address a request came from: Traefik's X-Real-Ip, else the peer's
// address; the zero Addr when neither reads as one.
func clientAddr(r *http.Request) netip.Addr {
	ip, err := netip.ParseAddr(clientIP(r))
	if err != nil {
		return netip.Addr{}
	}
	return ip.Unmap().WithZone("")
}

// clientIP is the text of the address a request came from: Traefik's X-Real-Ip, else the
// host of the peer's address. A poll session's socket carries it as its X-Real-Ip.
func clientIP(r *http.Request) string {
	if raw := strings.TrimSpace(r.Header.Get("X-Real-Ip")); raw != "" {
		return raw
	}
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
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
// slot 1, or in the slot it names (a space made anew after Phloem lost it: the device keeps the
// slot its writes are numbered in). 409 exists, 429 slow_down (per address and in all), 507 budget.
func (s *Server) createSpace(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Space  b64 `json:"space"`
		Token  b64 `json:"token"`
		Device b64 `json:"device"`
		Slot   int `json:"slot"`
	}
	if !s.readBody(w, r, 1<<10, &body) {
		return
	}
	if body.Slot == 0 {
		body.Slot = 1
	}
	if len(body.Space) != spaceLen || len(body.Token) != tokenLen || len(body.Device) != deviceLen || body.Slot < 1 || body.Slot > s.lim.MaxDevices {
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
	err := s.st.CreateSpace(r.Context(), body.Space, store.TokenHash(body.Token), body.Device, body.Slot, now)
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
	writeJSON(w, http.StatusCreated, map[string]int{"slot": body.Slot})
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

// leavePair is POST /sync/v1/pair: a sealed parcel for a device to come, kept for the seconds of
// its ttl (Limits.PairMinTTL to PairMaxTTL), else Limits.PairTTL. 201; 400 invalid (also a ttl
// beyond them); 409 exists (also the id of a parcel that was); 413 too_big; 429 slow_down (the
// space's pushes, or as many parcels waiting as it may have); 507 budget.
func (s *Server) leavePair(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	var body struct {
		ID  b64    `json:"id"`
		Box b64    `json:"box"`
		TTL *int64 `json:"ttl"`
	}
	if !s.readBody(w, r, int64(b64len(s.lim.MaxBox))+1<<10, &body) {
		return
	}
	if len(body.ID) != pairLen || body.Box == nil {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	ttl := s.lim.PairTTL
	if body.TTL != nil {
		// In seconds, compared before they become a duration, which a huge number would overflow.
		if *body.TTL < int64(s.lim.PairMinTTL/time.Second) || *body.TTL > int64(s.lim.PairMaxTTL/time.Second) {
			s.fail(w, http.StatusBadRequest, "invalid")
			return
		}
		ttl = time.Duration(*body.TTL) * time.Second
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
	err := s.st.PutPair(r.Context(), body.ID, sp.ID, body.Box, now, now.Add(ttl), s.lim.MaxPairs)
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

// takePair is GET /sync/v1/pair with Betula-Pair: the parcel once; the devices of its space
// online hear that it was taken. 200 {"box"}; 404 with what became of it (taken, expired,
// withdrawn), or not_found; 429 slow_down (Limits.TakesPerAddr).
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
	took, err := s.st.TakePair(r.Context(), id, now, s.lim.MaxPairsGone)
	switch {
	case errors.Is(err, store.ErrNotFound):
		s.m.parcels.Inc("missed")
		writeError(w, http.StatusNotFound, "not_found")
		return
	case err != nil:
		s.internal(w, "take", err)
		return
	case took.State != "":
		// 404 as before Phloem 0.2.0, so that an older browser reads it as it did; the code
		// says more.
		if took.Expired {
			s.m.parcels.Inc("expired")
		}
		s.m.parcels.Inc("missed")
		writeError(w, http.StatusNotFound, took.State)
		return
	}
	s.m.parcels.Inc("taken")
	s.tellRoom(took.Space, pairFrame(id, store.PairTaken))
	writeJSON(w, http.StatusOK, struct {
		Box b64 `json:"box"`
	}{took.Box})
}

// withdrawPair is DELETE /sync/v1/pair with Betula-Pair („Zurückziehen"): the space's parcel
// that waits is withdrawn from now on. 204, also for an id that is not the space's or waits no
// longer (nothing changes then).
func (s *Server) withdrawPair(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	id, ok := decode64(r.Header.Get("Betula-Pair"), pairLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	found, err := s.st.WithdrawPair(r.Context(), id, sp.ID, s.now(), s.lim.MaxPairsGone)
	if err != nil {
		s.internal(w, "withdraw", err)
		return
	}
	if found {
		s.m.parcels.Inc("withdrawn")
	}
	w.WriteHeader(http.StatusNoContent)
}

// listPairs is GET /sync/v1/pairs: the parcels of the space that wait (at: when they expire),
// and what became of the others (at: when it happened), newest first. 200 {"parcels": […]}.
func (s *Server) listPairs(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	list, err := s.st.Pairs(r.Context(), sp.ID, s.now())
	if err != nil {
		s.internal(w, "pairs", err)
		return
	}
	type parcel struct {
		ID    b64    `json:"id"`
		State string `json:"state"`
		At    int64  `json:"at"`
	}
	out := make([]parcel, 0, len(list))
	for _, p := range list {
		out = append(out, parcel{p.ID, p.State, p.At.UnixMilli()})
	}
	writeJSON(w, http.StatusOK, struct {
		Parcels []parcel `json:"parcels"`
	}{out})
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
