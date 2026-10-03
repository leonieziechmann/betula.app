package server

import (
	"context"
	"errors"
	"io"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/cortex/internal/cluster"
)

// newForwardTransport is the transport of a follower to its leader: on the same host or the
// same overlay network, so a connection that takes longer than 2 s is not coming; no proxy of
// the environment; the client's Accept-Encoding is passed on as it is (no gzip of its own).
func newForwardTransport() *http.Transport {
	dialer := &net.Dialer{Timeout: 2 * time.Second, KeepAlive: 30 * time.Second}
	return &http.Transport{
		Proxy:               nil,
		DialContext:         dialer.DialContext,
		MaxIdleConns:        64,
		MaxIdleConnsPerHost: 16,
		IdleConnTimeout:     90 * time.Second,
		DisableCompression:  true,
	}
}

// codeForwardFailed is the answer to a forwarded write whose leader failed after it got the
// request: it may have applied it, so the client must not simply send it again elsewhere (it
// does on 503 no-leader).
const codeForwardFailed = "forward-failed" // 502

// leaderPoll is how often a request that waits for a leader looks for one.
const leaderPoll = 100 * time.Millisecond

// forward hands a request this instance cannot answer to the leader (a reverse proxy, with
// the header Cortex-Forwarded: <this instance>), and answers with the leader's answer. It
// returns false when the request is to be handled here after all: this instance became the
// leader while it waited for one.
//
// A request that carries Cortex-Forwarded is never forwarded again: 503 no-leader. When no
// leader is known, or the leader cannot be reached, the instance waits up to LeaderWait for a
// leader: itself, or another one that announces itself meanwhile (a take-over), to which the
// request goes then. When none comes, a fetch with stale=if-error is answered with the stored
// version if there is one (Cache-Status detail=stale-if-error, Cortex-Upstream-Error:
// no-leader); else 503 no-leader with Retry-After: 1, on which a client tries the next
// instance. A write the leader may have applied before it failed (the request had reached it)
// is never answered no-leader, but 502 forward-failed.
func (s *Server) forward(w http.ResponseWriter, r *http.Request) bool {
	ri := info(r)
	self := s.node.Self().Instance
	if by := r.Header.Get("Cortex-Forwarded"); by != "" {
		s.noLeader(w, r, "forwarded by "+by+" to "+self+", which does not lead")
		return true
	}
	why := "no leader is known"
	var failed *cluster.Info
	for range 3 {
		leader, lead := s.awaitLeader(r.Context(), self, failed)
		if lead {
			ri.forwardedTo = ""
			return false
		}
		if leader == nil {
			break
		}
		target, err := url.Parse(leader.URL)
		if err != nil || target.Host == "" {
			why = "the leader's URL " + leader.URL + " is not usable"
			break
		}
		perr, sent := s.proxy(w, r, *leader, target)
		if perr == nil {
			return true
		}
		if sent && r.Method != http.MethodGet && r.Method != http.MethodHead {
			writeError(w, http.StatusBadGateway, codeForwardFailed, "the leader "+leader.Instance+" ("+leader.URL+
				") failed after it got the request, which it may have applied: "+perr.Error())
			return true
		}
		why = "the leader " + leader.Instance + " (" + leader.URL + ") cannot be reached: " + perr.Error()
		failed = leader
	}
	ri.forwardedTo = ""
	if s.staleIfNoLeader(w, r) {
		return true
	}
	s.noLeader(w, r, why)
	return true
}

// awaitLeader waits up to LeaderWait for a leader other than failed (the one that could not be
// reached): lead true when it is this instance, else the leader to forward to (nil when none
// came).
func (s *Server) awaitLeader(ctx context.Context, self string, failed *cluster.Info) (leader *cluster.Info, lead bool) {
	deadline := time.Now().Add(s.opt.LeaderWait)
	for {
		if l, ok := s.node.Leader(); ok && l.URL != "" && l.Instance != self &&
			(failed == nil || l.Instance != failed.Instance || l.URL != failed.URL || l.Epoch != failed.Epoch) {
			return &l, false
		}
		left := time.Until(deadline)
		if left <= 0 || ctx.Err() != nil {
			return nil, false
		}
		if s.node.WaitLeader(ctx, min(left, leaderPoll)) {
			return nil, true
		}
	}
}

// errNotLeaderThere is a forwarded request that the instance taken for the leader refused
// with 503 no-leader: it hands over, or the announcement named it after it stopped leading.
// It did not handle the request, so the follower goes on waiting for the leader.
var errNotLeaderThere = errors.New("it does not lead (503 no-leader)")

// proxy sends r to the leader and answers with the leader's answer. On a failure nothing has
// been answered: err says why, and sent whether the request may have reached the leader (more
// than the dial failed, or the leader read some of the body), so that it cannot be sent again.
// A 503 no-leader of the leader for a request whose body it did not read counts as a failure
// that sent nothing.
func (s *Server) proxy(w http.ResponseWriter, r *http.Request, leader cluster.Info, target *url.URL) (err error, sent bool) {
	ri := info(r)
	self := s.node.Self().Instance
	ri.forwardedTo = leader.URL
	if ri.result != "" {
		ri.result = "forwarded"
	}
	orig := r.Body
	body := &detachableBody{rc: orig}
	if orig != nil && orig != http.NoBody {
		r.Body = body
	}
	var proxyErr error
	proxy := &httputil.ReverseProxy{
		Rewrite: func(pr *httputil.ProxyRequest) {
			pr.SetURL(target)
			pr.Out.Host = target.Host
			pr.Out.Header.Set("Cortex-Forwarded", self)
		},
		Transport:     s.transmit,
		FlushInterval: -1,
		ModifyResponse: func(resp *http.Response) error {
			if resp.StatusCode == http.StatusServiceUnavailable && resp.Header.Get("Cortex-Error") == codeNoLeader && body.count() == 0 {
				return errNotLeaderThere
			}
			return nil
		},
		ErrorHandler: func(_ http.ResponseWriter, _ *http.Request, err error) { proxyErr = err },
	}
	// The leader's answer names the leader.
	instance := w.Header().Values("Cortex-Instance")
	w.Header().Del("Cortex-Instance")
	proxy.ServeHTTP(w, r)
	read := body.detach() > 0
	r.Body = orig
	if proxyErr == nil {
		return nil, true
	}
	w.Header()["Cortex-Instance"] = instance
	if errors.Is(proxyErr, errNotLeaderThere) {
		return proxyErr, false
	}
	var op *net.OpError
	dialFailed := errors.As(proxyErr, &op) && op.Op == "dial"
	return proxyErr, read || !dialFailed
}

// staleIfNoLeader answers a fetch that needs the leader, when none can be reached, with the
// version this instance has stored, as the leader would when upstream fails: under
// stale=if-error (the default), never for at (a version of the past is not the current one).
// false when it did not answer.
func (s *Server) staleIfNoLeader(w http.ResponseWriter, r *http.Request) bool {
	if r.URL.Path != "/v1/fetch" || (r.Method != http.MethodGet && r.Method != http.MethodHead) {
		return false
	}
	p, err := parseFetch(r)
	if err != nil || p.stale != staleIfError || !p.at.IsZero() {
		return false
	}
	_, v, err := s.st.Lookup(p.key)
	if err != nil || !s.usable(v, p.expect) {
		return false
	}
	s.serve(w, r, v, "Cortex; hit; detail=stale-if-error", http.Header{"Cortex-Upstream-Error": {codeNoLeader}}, "stale_if_error")
	return true
}

// beginWrite starts a write of a request that has not read its body: inside the node's write
// fence while this instance leads. On a follower it forwards the request and returns nil, as
// it does when no leader can take it (503 no-leader).
func (s *Server) beginWrite(w http.ResponseWriter, r *http.Request) (done func()) {
	for range 2 {
		if done, ok := s.node.BeginWrite(); ok {
			return done
		}
		if s.forward(w, r) {
			return nil
		}
		// forward found that this instance leads (again): try the fence once more.
	}
	s.noLeader(w, r, "the role of this instance keeps changing")
	return nil
}

// noLeader answers 503 no-leader: the client tries the next instance after a second.
func (s *Server) noLeader(w http.ResponseWriter, r *http.Request, message string) {
	if ri := info(r); ri.result != "" {
		ri.result = "error"
	}
	writeErrorRetry(w, http.StatusServiceUnavailable, codeNoLeader, time.Second, message)
}

// detachableBody passes a request body to the reverse proxy without letting it close the body,
// and cuts the proxy off afterwards, so that a body the leader never read can still be read
// here.
type detachableBody struct {
	mu       sync.Mutex
	rc       io.ReadCloser
	n        int64
	detached bool
}

var errDetached = errors.New("the request body was taken back from the forwarding")

func (b *detachableBody) Read(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.detached {
		return 0, errDetached
	}
	n, err := b.rc.Read(p)
	b.n += int64(n)
	return n, err
}

// Close does not close the request body: the server does that when the handler returns.
func (b *detachableBody) Close() error { return nil }

// count is how many bytes the proxy has read so far.
func (b *detachableBody) count() int64 {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.n
}

// detach ends the proxy's reading and returns how many bytes it had read.
func (b *detachableBody) detach() int64 {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.detached = true
	return b.n
}

// codeBlobMissing is the answer to a read of a version whose blob this instance lacks while it
// leads, when the other instance cannot serve it either: a follower promoted before its
// back-fill ended fetches the blob from the other instance, so the read may succeed later
// (503 with Retry-After), and it is not Cortex failing (no 500; E2E-4).
const codeBlobMissing = "blob-missing" // 503

// fromPeer answers a read whose blob this leader lacks with the other instance's answer (a
// reverse proxy, as forward), when the node knows the other instance (cluster.PeerNode). The
// request carries Cortex-Forwarded, so the other instance answers from its own copy or, when it
// lacks the blob too, 503 no-leader, which counts as not answered. false when it did not
// answer: then nothing has been written.
func (s *Server) fromPeer(w http.ResponseWriter, r *http.Request) bool {
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		return false
	}
	if r.Header.Get("Cortex-Forwarded") != "" { // the other instance asks this one: never back
		return false
	}
	pn, ok := s.node.(cluster.PeerNode)
	if !ok {
		return false
	}
	peer, ok := pn.Peer()
	if !ok {
		return false
	}
	target, err := url.Parse(peer.URL)
	if err != nil || target.Host == "" {
		return false
	}
	perr, _ := s.proxy(w, r, peer, target)
	if perr != nil {
		info(r).forwardedTo = ""
		return false
	}
	return true
}

// blobMissing answers a read whose blob this instance lacks: a follower asks its leader, a
// leader the other instance; when neither answers, 503 blob-missing.
func (s *Server) blobMissing(w http.ResponseWriter, r *http.Request, hash string) {
	if !s.leading() && s.forward(w, r) {
		return
	}
	if s.leading() && s.fromPeer(w, r) {
		return
	}
	if ri := info(r); ri.result != "" {
		ri.result = "error"
	}
	writeErrorRetry(w, http.StatusServiceUnavailable, codeBlobMissing, time.Second,
		"the content sha256:"+hash+" is not here yet: the other instance did not serve it, and it is being fetched from there")
}
