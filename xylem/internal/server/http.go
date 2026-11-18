package server

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// bodyTimeout bounds the reading of a request body.
const bodyTimeout = 30 * time.Second

// maxBody is the largest body of a request: a subscription with its topics is a kilobyte, an
// announcement in eight languages a few.
const maxBody = 32 << 10

var (
	// topicName is a topic: lower-case letters, digits and hyphens, a letter first, 32 at most.
	topicName = regexp.MustCompile(`^[a-z][a-z0-9-]{0,31}$`)
	// langName is a language: its two or three letters, and a region or script if it names one.
	langName = regexp.MustCompile(`^[a-z]{2,3}(-[a-z0-9]{2,8})?$`)
)

// Handler is Xylem's HTTP API: /push/v1/ (Traefik routes it), /healthz, and for the commands of
// the same machine /status and /announcements (they answer loopback only).
func (s *Server) Handler() http.Handler { return http.HandlerFunc(s.serveHTTP) }

func (s *Server) serveHTTP(w http.ResponseWriter, r *http.Request) {
	h := w.Header()
	h.Set("Cache-Control", "no-store")
	h.Set("X-Content-Type-Options", "nosniff")
	path, method := r.URL.Path, r.Method
	if strings.HasPrefix(path, "/push/") {
		h.Add("Vary", "Origin")
		if origin := r.Header.Get("Origin"); origin != "" {
			if !s.originAllowed(origin, r.Host) {
				s.fail(w, http.StatusForbidden, "origin")
				return
			}
			h.Set("Access-Control-Allow-Origin", origin)
		}
		if method == http.MethodOptions {
			h.Set("Access-Control-Allow-Methods", "GET, PUT, DELETE")
			h.Set("Access-Control-Allow-Headers", "Content-Type")
			h.Set("Access-Control-Max-Age", "600")
			w.WriteHeader(http.StatusNoContent)
			return
		}
	}
	switch {
	case path == "/push/v1/key" && method == http.MethodGet:
		writeJSON(w, http.StatusOK, map[string]string{"key": s.key.Public()})
	case path == "/push/v1/subscription" && method == http.MethodPut:
		s.putSubscription(w, r)
	case path == "/push/v1/subscription" && method == http.MethodDelete:
		s.deleteSubscription(w, r)
	case path == "/healthz" && (method == http.MethodGet || method == http.MethodHead):
		s.healthz(w, r)
	case path == "/push/v1/key", path == "/push/v1/subscription", path == "/healthz":
		h.Set("Allow", allowed(path))
		writeError(w, http.StatusMethodNotAllowed, "method")
	case path == "/status" || path == "/announcements" || strings.HasPrefix(path, "/announcements/"):
		// The commands' side: not there at all for anyone who is not this machine.
		if !fromHere(r) {
			writeError(w, http.StatusNotFound, "not_found")
			return
		}
		s.serveLocal(w, r)
	default:
		writeError(w, http.StatusNotFound, "not_found")
	}
}

func allowed(path string) string {
	switch path {
	case "/push/v1/key":
		return "GET, OPTIONS"
	case "/push/v1/subscription":
		return "PUT, DELETE, OPTIONS"
	}
	return "GET, HEAD"
}

// originAllowed says whether a browser of origin may use Xylem: one of Options.Origins, or
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

// readBody reads a JSON body of at most maxBody bytes into v; false after it answered.
func (s *Server) readBody(w http.ResponseWriter, r *http.Request, v any) bool {
	_ = http.NewResponseController(w).SetReadDeadline(time.Now().Add(bodyTimeout))
	data, err := io.ReadAll(http.MaxBytesReader(w, r.Body, maxBody))
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

// b64 is bytes in base64url, as a browser writes the keys of a subscription (without padding;
// with it they read as well).
type b64 []byte

// UnmarshalJSON implements json.Unmarshaler.
func (b *b64) UnmarshalJSON(raw []byte) error {
	if string(raw) == "null" {
		*b = nil
		return nil
	}
	if len(raw) < 2 || raw[0] != '"' || raw[len(raw)-1] != '"' || bytes.IndexByte(raw, '\\') >= 0 {
		return errors.New("not base64url")
	}
	text := bytes.TrimRight(raw[1:len(raw)-1], "=")
	out := make([]byte, base64.RawURLEncoding.DecodedLen(len(text)))
	n, err := base64.RawURLEncoding.Decode(out, text)
	if err != nil {
		return errors.New("not base64url")
	}
	*b = out[:n]
	return nil
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

// fromHere says whether a request comes from this machine and from a command, not a page: a
// loopback peer, no header a proxy would have added, and no Origin (a page open in a browser of
// this machine could otherwise ask too; a browser names its origin with every request that
// changes something, and with the JSON these take).
func fromHere(r *http.Request) bool {
	if r.Header.Get("X-Real-Ip") != "" || r.Header.Get("X-Forwarded-For") != "" || r.Header.Get("Origin") != "" {
		return false
	}
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return false
	}
	ip, err := netip.ParseAddr(host)
	return err == nil && ip.IsLoopback()
}

// slowDown answers 429 with when to come back.
func (s *Server) slowDown(w http.ResponseWriter) {
	w.Header().Set("Retry-After", "600")
	s.fail(w, http.StatusTooManyRequests, "slow_down")
}

// putSubscription is PUT /push/v1/subscription: a browser's push subscription as the browser
// writes it (endpoint, keys), with the language of its texts and the topics it listens to. Put
// again, it replaces what Xylem held for the endpoint; with no topic it is forgotten. 204.
// 400 invalid, endpoint (no push service's Xylem writes to), keys, lang, topics; 429 slow_down;
// 507 full.
func (s *Server) putSubscription(w http.ResponseWriter, r *http.Request) {
	addr := clientKey(r)
	now := s.now()
	if !s.writes.take(addr, 1, now) {
		s.slowDown(w)
		return
	}
	var body struct {
		Endpoint string `json:"endpoint"`
		Keys     struct {
			P256dh b64 `json:"p256dh"`
			Auth   b64 `json:"auth"`
		} `json:"keys"`
		Lang   string   `json:"lang"`
		Topics []string `json:"topics"`
	}
	if !s.readBody(w, r, &body) {
		return
	}
	ep, err := s.endpointOf(body.Endpoint)
	if err != nil {
		s.refused(ep.host)
		s.fail(w, http.StatusBadRequest, "endpoint")
		return
	}
	if webpush.CheckKeys(body.Keys.P256dh, body.Keys.Auth) != nil {
		s.fail(w, http.StatusBadRequest, "keys")
		return
	}
	lang := strings.ToLower(strings.TrimSpace(body.Lang))
	if lang == "" {
		lang = s.lang
	}
	if !langName.MatchString(lang) {
		s.fail(w, http.StatusBadRequest, "lang")
		return
	}
	topics, ok := cleanTopics(body.Topics, s.lim.MaxTopics)
	if !ok {
		s.fail(w, http.StatusBadRequest, "topics")
		return
	}
	if len(topics) == 0 {
		s.forget(w, r, body.Endpoint)
		return
	}
	// A new subscription counts against what one address, and all of them, may make in an hour;
	// one Xylem holds already is only brought up to date.
	counted := s.news.take(addr, 1, now)
	if counted && !s.newAll.take("", 1, now) {
		s.news.give(addr, 1)
		counted = false
	}
	uncount := func() { // not new after all: what it took of the new ones goes back
		if counted {
			s.news.give(addr, 1)
			s.newAll.give("", 1)
		}
	}
	if !counted {
		_, _, found, err := s.st.Topics(r.Context(), body.Endpoint)
		if err != nil {
			s.internal(w, "topics", err)
			return
		}
		if !found {
			s.slowDown(w)
			return
		}
	}
	made, err := s.st.Put(r.Context(), store.Subscription{
		Endpoint: body.Endpoint, P256dh: body.Keys.P256dh, Auth: body.Keys.Auth, Lang: lang, Topics: topics,
	}, now, s.max)
	if errors.Is(err, store.ErrFull) {
		uncount()
		s.mu.Lock()
		first := !s.full
		s.full = true
		s.mu.Unlock()
		if first {
			oplog.For("subscriptions").Warn("no new subscription is taken: Xylem holds as many as it may", "event", "subscriptions.full", "max", s.max)
		}
		s.fail(w, http.StatusInsufficientStorage, "full")
		return
	}
	if err != nil {
		uncount()
		s.internal(w, "put", err)
		return
	}
	if made {
		s.m.subs.Inc("added", ep.service)
	} else {
		s.m.subs.Inc("updated", ep.service)
		uncount()
	}
	w.WriteHeader(http.StatusNoContent)
}

// cleanTopics checks the topics of a subscription and returns them once each, sorted.
func cleanTopics(topics []string, max int) ([]string, bool) {
	seen := map[string]bool{}
	var out []string
	for _, t := range topics {
		if !topicName.MatchString(t) {
			return nil, false
		}
		if !seen[t] {
			seen[t] = true
			out = append(out, t)
		}
	}
	if len(out) > max {
		return nil, false
	}
	sort.Strings(out)
	return out, true
}

// deleteSubscription is DELETE /push/v1/subscription: the subscription of the endpoint in the
// body is forgotten. 204 whether there was one or not: who knows an endpoint may end it, and
// learns nothing from asking.
func (s *Server) deleteSubscription(w http.ResponseWriter, r *http.Request) {
	if !s.writes.take(clientKey(r), 1, s.now()) {
		s.slowDown(w)
		return
	}
	var body struct {
		Endpoint string `json:"endpoint"`
	}
	if !s.readBody(w, r, &body) {
		return
	}
	if body.Endpoint == "" || len(body.Endpoint) > s.lim.MaxEndpoint {
		s.fail(w, http.StatusBadRequest, "endpoint")
		return
	}
	s.forget(w, r, body.Endpoint)
}

func (s *Server) forget(w http.ResponseWriter, r *http.Request, endpoint string) {
	found, err := s.st.Remove(r.Context(), endpoint)
	if err != nil {
		s.internal(w, "remove", err)
		return
	}
	if found {
		service := otherService
		if ep, err := s.endpointOf(endpoint); err == nil {
			service = ep.service
		}
		s.m.subs.Inc("removed", service)
		s.mu.Lock()
		s.full = false
		s.mu.Unlock()
	}
	w.WriteHeader(http.StatusNoContent)
}

// healthz is GET /healthz: 200 while the store answers.
func (s *Server) healthz(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	if err := s.st.Ping(r.Context()); err != nil {
		w.WriteHeader(http.StatusServiceUnavailable)
		_, _ = io.WriteString(w, "store: "+err.Error()+"\n")
		return
	}
	_, _ = io.WriteString(w, "ok\n")
}

// retryAfter reads a Retry-After header in seconds (the form the push services send); 0: none.
func retryAfter(h http.Header) time.Duration {
	n, err := strconv.Atoi(strings.TrimSpace(h.Get("Retry-After")))
	if err != nil || n <= 0 {
		return 0
	}
	return time.Duration(n) * time.Second
}
