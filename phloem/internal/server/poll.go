package server

import (
	"bufio"
	"context"
	"crypto/rand"
	"encoding/json"
	"errors"
	"io"
	"log"
	"net"
	"net/http"
	"strconv"
	"sync"
	"time"

	"github.com/coder/websocket"
)

// The socket over HTTPS (docs/phloem/protocol.md section 4, „The socket over HTTPS", since
// Phloem 0.4.0), for a network that breaks the WebSocket's upgrade but lets plain HTTPS
// requests through (a proxy, an antivirus that looks into TLS). A poll session is a bridge:
// Phloem opens a WebSocket to its own serveWS in memory (the ends of a net.Pipe: no network
// socket, which its firewall forbids), as the device's, with its address and origin, and the
// device's requests carry the frames: POST /sync/v1/poll writes them into that socket, GET
// /sync/v1/poll takes what came out of it. The socket behind a session is one like any other
// (admitted, counted, pinged, held to its queue), so every rule of section 4 holds for both.

const (
	pollLen = 16 // bytes of a poll session's id
	// pollSlack is what a body of frames may hold beyond the largest frame a socket reads, so
	// that a frame above that limit still reaches the socket, which closes with 1009 as it would.
	pollSlack = 512 << 10
	// pollURL is what the bridge dials; the host names nothing on any network.
	pollURL = "ws://phloem/sync/v1/ws"
)

// pollClose is how the socket behind a poll session ended: the code and reason of its close,
// or 0 and "" when it ended without one (lost, or let go).
type pollClose struct {
	Code   int    `json:"code"`
	Reason string `json:"reason"`
}

// polled is a frame that waits for the device's GET.
type polled struct {
	data    []byte
	release func() // gives back its place among the frames held
}

func freePolled(frames []polled) {
	for _, f := range frames {
		f.release()
	}
}

// poll is one poll session: the device's end of a socket in memory, and what waits for it.
type poll struct {
	s      *Server
	id     string
	conn   *websocket.Conn
	ctx    context.Context // ends with the socket: the reads and writes of the bridge end
	cancel context.CancelFunc
	write  sync.Mutex // one POST at a time writes into the socket, its frames in order

	mu     sync.Mutex
	queue  []polled      // the frames for the device, in order
	bytes  int64         // their bytes
	read   bool          // a frame came: the first (the welcome) is not held to the queue's bounds
	end    *pollClose    // how the socket ended, once it did
	wake   chan struct{} // closed when a frame or the end comes, then made anew
	getter chan struct{} // the GET that waits; closed when a newer one comes
	asking int           // requests in flight
	seen   time.Time     // when the last request ended
	gone   bool          // out of the table: every request answers 404 gone
	idle   *time.Timer
}

// postPoll is POST /sync/v1/poll: without Betula-Poll it opens a session with the frames of
// its body, a hello first (201 {"poll"}); with it, it writes them into the session's socket
// (204).
func (s *Server) postPoll(w http.ResponseWriter, r *http.Request) {
	if r.Header.Get("Betula-Poll") == "" {
		s.openPoll(w, r)
		return
	}
	p := s.pollOf(w, r)
	if p == nil {
		return
	}
	defer p.finish()
	frames, release, tooBig, ok := s.readFrames(w, r)
	if !ok {
		if tooBig { // as a socket that got a frame too big
			p.mu.Lock()
			p.endLocked(pollClose{Code: int(websocket.StatusMessageTooBig), Reason: "too big"}, "closed")
			p.mu.Unlock()
			_ = p.conn.CloseNow()
		}
		return
	}
	p.deliver(frames)
	release()
	w.WriteHeader(http.StatusNoContent)
}

// openPoll opens a poll session: its socket, admitted like any (503 busy), gets the frames of
// the body, a hello first (400 invalid otherwise); what follows comes through GET.
func (s *Server) openPoll(w http.ResponseWriter, r *http.Request) {
	if s.isShut() { // as serveWS answers
		s.fail(w, http.StatusServiceUnavailable, "busy")
		return
	}
	frames, release, _, ok := s.readFrames(w, r)
	if !ok {
		return
	}
	defer release()
	var hello inFrame
	if len(frames) == 0 || json.Unmarshal([]byte(frames[0]), &hello) != nil || !hello.isHello() {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	p, status := s.dialPoll(r)
	switch {
	case status == http.StatusServiceUnavailable:
		writeError(w, status, "busy") // serveWS counted it
		return
	case p == nil:
		writeError(w, http.StatusInternalServerError, "internal")
		return
	}
	p.deliver(frames)
	p.finish()
	writeJSON(w, http.StatusCreated, map[string]string{"poll": enc.EncodeToString([]byte(p.id))})
}

// readFrames reads a body {"frames": ["<frame>"…]} of at most what a socket reads in one frame
// and a little more; false after it answered (413 too_big, which tooBig says, or 400 invalid).
// A body beyond bigFrame waits for a place among Server.bodies, which release gives back.
func (s *Server) readFrames(w http.ResponseWriter, r *http.Request) (frames []string, release func(), tooBig, ok bool) {
	_ = http.NewResponseController(w).SetReadDeadline(time.Now().Add(s.lim.FrameTimeout))
	data, release, err := readHeld(r.Context(), http.MaxBytesReader(w, r.Body, s.lim.readLimit()+pollSlack), s.bodies)
	if err != nil {
		var max *http.MaxBytesError
		if errors.As(err, &max) {
			s.fail(w, http.StatusRequestEntityTooLarge, "too_big")
			return nil, nil, true, false
		}
		s.fail(w, http.StatusBadRequest, "invalid")
		return nil, nil, false, false
	}
	var body struct {
		Frames []string `json:"frames"`
	}
	if json.Unmarshal(data, &body) != nil || body.Frames == nil {
		release()
		s.fail(w, http.StatusBadRequest, "invalid")
		return nil, nil, false, false
	}
	return body.Frames, release, false, true
}

// dialPoll opens a session's socket: a WebSocket to serveWS in memory, as the device's (its
// address, so that the limits and TURN see the device, and its origin), admitted like any
// socket. nil, and the status to answer, when it gets none.
func (s *Server) dialPoll(r *http.Request) (*poll, int) {
	hdr := http.Header{"X-Real-Ip": {clientIP(r)}}
	if origin := r.Header.Get("Origin"); origin != "" {
		hdr.Set("Origin", origin)
	}
	ctx, cancel := context.WithTimeout(s.ctx, s.lim.WriteTimeout)
	conn, resp, err := websocket.Dial(ctx, pollURL, &websocket.DialOptions{
		HTTPClient: s.bridge.client, HTTPHeader: hdr, CompressionMode: websocket.CompressionDisabled,
	})
	cancel()
	if err != nil {
		if resp != nil && resp.StatusCode == http.StatusServiceUnavailable {
			return nil, http.StatusServiceUnavailable
		}
		return nil, http.StatusInternalServerError
	}
	conn.SetReadLimit(-1) // Phloem's own frames, bounded where they are made
	pctx, pcancel := context.WithCancel(s.ctx)
	p := &poll{s: s, conn: conn, ctx: pctx, cancel: pcancel, wake: make(chan struct{}), asking: 1, seen: time.Now()}
	p.mu.Lock()
	p.idle = time.AfterFunc(s.lim.PollIdle, p.checkIdle)
	p.mu.Unlock()
	s.pollMu.Lock()
	for p.id == "" || s.polls[p.id] != nil {
		p.id = string(randomBytes(pollLen))
	}
	s.polls[p.id] = p
	s.pollMu.Unlock()
	s.pollsLive.Add(1)
	s.m.pollsOpened.Inc()
	// The socket's serveWS waits for the hello and holds its place in s.wg meanwhile, so the
	// group cannot be at zero here.
	s.wg.Add(1)
	go p.run()
	return p, 0
}

// pollOf is the session the request's Betula-Poll names, counted as asking until its finish;
// nil after it answered (400 invalid, 404 gone).
func (s *Server) pollOf(w http.ResponseWriter, r *http.Request) *poll {
	id, ok := decode64(r.Header.Get("Betula-Poll"), pollLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return nil
	}
	s.pollMu.Lock()
	p := s.polls[string(id)]
	s.pollMu.Unlock()
	if p == nil || !p.begin() {
		writeError(w, http.StatusNotFound, "gone")
		return nil
	}
	return p
}

// dropPoll takes a session out of the table.
func (s *Server) dropPoll(p *poll) {
	s.pollMu.Lock()
	if s.polls[p.id] == p {
		delete(s.polls, p.id)
	}
	s.pollMu.Unlock()
}

// endPolls forgets every session when the server is closed (their sockets ended before).
func (s *Server) endPolls() {
	s.pollMu.Lock()
	polls := s.polls
	s.polls = map[string]*poll{}
	s.pollMu.Unlock()
	for _, p := range polls {
		p.mu.Lock()
		p.gone = true
		p.idle.Stop()
		frames := p.queue
		p.queue, p.bytes = nil, 0
		p.endLocked(pollClose{}, "lost")
		p.mu.Unlock()
		freePolled(frames)
		_ = p.conn.CloseNow()
	}
}

// getPoll is GET /sync/v1/poll: the frames that came for the device, at once when some wait,
// else as soon as one comes, at the latest after Limits.PollWait none; with the close once the
// socket behind it ended, and then the session is gone. A newer GET ends the one that waits.
func (s *Server) getPoll(w http.ResponseWriter, r *http.Request) {
	p := s.pollOf(w, r)
	if p == nil {
		return
	}
	defer p.finish()
	mine := make(chan struct{})
	p.mu.Lock()
	if p.getter != nil {
		close(p.getter)
	}
	p.getter = mine
	p.mu.Unlock()
	wait := time.NewTimer(s.lim.PollWait)
	defer wait.Stop()
	for {
		p.mu.Lock()
		if p.getter != mine { // a newer GET took over
			p.mu.Unlock()
			s.writePolled(w, nil, nil)
			return
		}
		if len(p.queue) > 0 || p.end != nil {
			frames, end := p.queue, p.end
			p.queue, p.bytes, p.getter = nil, 0, nil
			if end != nil {
				p.gone = true
				p.idle.Stop()
			}
			p.mu.Unlock()
			if end != nil {
				s.dropPoll(p)
			}
			s.writePolled(w, frames, end)
			freePolled(frames)
			return
		}
		wake := p.wake
		p.mu.Unlock()
		select {
		case <-wake:
		case <-mine:
		case <-wait.C:
			p.letGo(mine)
			s.writePolled(w, nil, nil)
			return
		case <-r.Context().Done():
			p.letGo(mine)
			return
		}
	}
}

// letGo: the GET of mine waits no more.
func (p *poll) letGo(mine chan struct{}) {
	p.mu.Lock()
	if p.getter == mine {
		p.getter = nil
	}
	p.mu.Unlock()
}

// writePolled answers a GET: {"frames": [...]}, and "close" once the socket ended. The frames
// are Phloem's own JSON, written as strings without a copy of them.
func (s *Server) writePolled(w http.ResponseWriter, frames []polled, end *pollClose) {
	_ = http.NewResponseController(w).SetWriteDeadline(time.Now().Add(s.lim.WriteTimeout))
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	bw := bufio.NewWriterSize(w, 32<<10)
	_, _ = bw.WriteString(`{"frames":[`)
	for i, f := range frames {
		if i > 0 {
			_ = bw.WriteByte(',')
		}
		writeQuoted(bw, f.data)
	}
	_ = bw.WriteByte(']')
	if end != nil {
		reason, _ := json.Marshal(end.Reason)
		_, _ = bw.WriteString(`,"close":{"code":`)
		_, _ = bw.WriteString(strconv.Itoa(end.Code))
		_, _ = bw.WriteString(`,"reason":`)
		_, _ = bw.Write(reason)
		_ = bw.WriteByte('}')
	}
	_, _ = bw.WriteString("}\n")
	_ = bw.Flush()
}

// writeQuoted writes text as a JSON string. Phloem's frames are JSON already (valid UTF-8, no
// control characters), so quotes and backslashes are what they need escaped; a control
// character is escaped all the same.
func writeQuoted(bw *bufio.Writer, text []byte) {
	const hex = "0123456789abcdef"
	_ = bw.WriteByte('"')
	start := 0
	for i, c := range text {
		if c != '"' && c != '\\' && c >= 0x20 {
			continue
		}
		_, _ = bw.Write(text[start:i])
		if c == '"' || c == '\\' {
			_ = bw.WriteByte('\\')
			_ = bw.WriteByte(c)
		} else {
			_, _ = bw.WriteString(`\u00`)
			_ = bw.WriteByte(hex[c>>4])
			_ = bw.WriteByte(hex[c&0xf])
		}
		start = i + 1
	}
	_, _ = bw.Write(text[start:])
	_ = bw.WriteByte('"')
}

// begin counts a request of the session; false once it is gone.
func (p *poll) begin() bool {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.gone {
		return false
	}
	p.asking++
	return true
}

// finish: a request of the session ended; Limits.PollIdle without one from now, and it ends.
func (p *poll) finish() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.asking--
	p.seen = time.Now()
	if !p.gone {
		p.idle.Reset(p.s.lim.PollIdle)
	}
}

// checkIdle ends a session nobody asked anything for Limits.PollIdle, as a socket that stopped
// answering: its socket closes without a word, and the session is gone.
func (p *poll) checkIdle() {
	idle := p.s.lim.PollIdle
	p.mu.Lock()
	if p.gone {
		p.mu.Unlock()
		return
	}
	if left := idle - time.Since(p.seen); p.asking > 0 || left > 0 {
		if p.asking > 0 {
			left = idle // finish sets it anew
		}
		p.idle.Reset(left)
		p.mu.Unlock()
		return
	}
	p.gone = true
	frames := p.queue
	p.queue, p.bytes = nil, 0
	p.endLocked(pollClose{}, "idle")
	p.mu.Unlock()
	freePolled(frames)
	p.s.dropPoll(p)
	_ = p.conn.CloseNow()
}

// deliver writes frames into the socket in order, as the device would on a socket of its own.
// Once the socket is gone the rest is dropped, as a socket that closes drops what is sent: the
// GET says how it closed.
func (p *poll) deliver(frames []string) {
	p.write.Lock()
	defer p.write.Unlock()
	for _, f := range frames {
		ctx, cancel := context.WithTimeout(p.ctx, p.s.lim.WriteTimeout)
		err := p.conn.Write(ctx, websocket.MessageText, []byte(f))
		cancel()
		if err != nil {
			return
		}
	}
}

// run reads the socket's frames into the queue until the socket ends.
func (p *poll) run() {
	defer p.s.wg.Done()
	for {
		data, release, err := p.next()
		if err != nil {
			p.closed(err)
			return
		}
		p.push(data, release)
	}
}

// next reads one frame of Phloem's; beyond bigFrame it waits for a place among Server.held,
// which it keeps until a GET handed the frame out.
func (p *poll) next() ([]byte, func(), error) {
	typ, r, err := p.conn.Reader(p.ctx)
	if err != nil {
		return nil, nil, err
	}
	if typ != websocket.MessageText {
		return nil, nil, errBinary
	}
	return readHeld(p.ctx, r, p.s.held)
}

// push queues a frame for the device. A device too far behind is let go, as a socket would
// be: it comes back with since.
func (p *poll) push(data []byte, release func()) {
	lim := p.s.lim
	p.mu.Lock()
	if p.end != nil {
		p.mu.Unlock()
		release()
		return
	}
	if p.read && (len(p.queue) >= lim.QueueFrames || p.bytes+int64(len(data)) > lim.QueueBytes) {
		dropped := append(p.queue, polled{data, release})
		p.queue, p.bytes = nil, 0
		p.endLocked(pollClose{}, "slow")
		p.mu.Unlock()
		freePolled(dropped)
		_ = p.conn.CloseNow()
		return
	}
	p.read = true
	p.queue = append(p.queue, polled{data, release})
	p.bytes += int64(len(data))
	p.wakeLocked()
	p.mu.Unlock()
}

// closed notes how the socket ended: with its close frame, or without one.
func (p *poll) closed(err error) {
	end, reason := pollClose{}, "lost"
	var ce websocket.CloseError
	if errors.As(err, &ce) {
		end, reason = pollClose{Code: int(ce.Code), Reason: ce.Reason}, "closed"
	}
	p.mu.Lock()
	p.endLocked(end, reason)
	p.mu.Unlock()
	_ = p.conn.CloseNow()
}

// endLocked notes, once, how the socket ended: the GET that waits, or the next, hands it out.
// reason is the one of phloem_poll_sessions_ended_total. Under p.mu.
func (p *poll) endLocked(end pollClose, reason string) {
	if p.end != nil {
		return
	}
	p.end = &end
	p.s.pollsLive.Add(-1)
	p.s.m.pollsEnded.Inc(reason)
	p.cancel()
	p.wakeLocked()
}

func (p *poll) wakeLocked() {
	close(p.wake)
	p.wake = make(chan struct{})
}

// ---- the bridge ----

// bridge serves the sockets of the poll sessions in memory: an HTTP server on a listener of
// net.Pipe ends, and the client that dials it. Nothing of it touches the network.
type bridge struct {
	ln     *pipeListener
	hs     *http.Server
	client *http.Client
	served chan struct{}
}

func newBridge(h http.Handler) *bridge {
	ln := &pipeListener{conns: make(chan net.Conn), done: make(chan struct{})}
	b := &bridge{
		ln: ln,
		hs: &http.Server{Handler: h, ErrorLog: log.New(io.Discard, "", 0)},
		client: &http.Client{Transport: &http.Transport{
			DialContext: ln.dial, DisableKeepAlives: true, DisableCompression: true,
		}},
		served: make(chan struct{}),
	}
	go func() {
		defer close(b.served)
		_ = b.hs.Serve(ln)
	}()
	return b
}

// close ends the bridge's server; the sockets it upgraded are the sessions' own and ended
// before.
func (b *bridge) close() {
	_ = b.hs.Close()
	<-b.served
}

// pipeListener is a net.Listener of in-memory connections: dial hands one end of a net.Pipe to
// Accept and keeps the other.
type pipeListener struct {
	conns chan net.Conn
	done  chan struct{}
	once  sync.Once
}

func (l *pipeListener) Accept() (net.Conn, error) {
	select {
	case c := <-l.conns:
		return c, nil
	case <-l.done:
		return nil, net.ErrClosed
	}
}

func (l *pipeListener) Close() error {
	l.once.Do(func() { close(l.done) })
	return nil
}

func (l *pipeListener) Addr() net.Addr { return pipeAddr{} }

func (l *pipeListener) dial(ctx context.Context, _, _ string) (net.Conn, error) {
	client, server := net.Pipe()
	select {
	case l.conns <- server:
		return client, nil
	case <-l.done:
	case <-ctx.Done():
	}
	_ = client.Close()
	_ = server.Close()
	return nil, net.ErrClosed
}

type pipeAddr struct{}

func (pipeAddr) Network() string { return "pipe" }
func (pipeAddr) String() string  { return "phloem" }

// randomBytes is n random bytes.
func randomBytes(n int) []byte {
	b := make([]byte, n)
	_, _ = rand.Read(b)
	return b
}
