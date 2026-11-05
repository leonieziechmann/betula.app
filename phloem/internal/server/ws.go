package server

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"slices"
	"sync"
	"sync/atomic"
	"time"
	"unicode/utf8"

	"github.com/coder/websocket"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

const (
	// bigFrame: a frame larger than this, read or written, takes one of Limits.BigFrames while it
	// is in memory, so that a crowd of snapshots cannot take all of it.
	bigFrame = 128 << 10
	// helloLimit is the read limit until the hello (some 150 bytes).
	helloLimit = 4 << 10
	// snapshotUnit: a snapshot counts as one push for every unit of its data begun.
	snapshotUnit = 64 << 10
)

// The close codes of Phloem (docs/phloem/protocol.md section 4).
const (
	statusNormal    = websocket.StatusNormalClosure   // 1000: left, gone
	statusGoingAway = websocket.StatusGoingAway       // 1001: Phloem restarts
	statusPolicy    = websocket.StatusPolicyViolation // 1008: unauthorized, full, a frame against the protocol
	statusInternal  = websocket.StatusInternalError   // 1011
	statusTryLater  = websocket.StatusTryAgainLater   // 1013: Phloem is busy
	statusReplaced  = websocket.StatusCode(4000)      // a newer socket of the same device
)

// room is a space with sockets: its writes and the frames they cause happen under mu, in one
// order for every device.
type room struct {
	key  string
	refs int // sockets and calls that hold it; guarded by Server.mu

	mu    sync.Mutex
	peers map[string]*peer // by device id
	gone  bool             // the space was deleted: the room takes nobody
}

// slots are the slots online, ascending.
func (r *room) slots() []int {
	out := make([]int, 0, len(r.peers))
	for _, p := range r.peers {
		out = append(out, p.slot)
	}
	slices.Sort(out)
	return out
}

// presence tells every device but except who is online. Under r.mu.
func (r *room) presence(except *peer) {
	f := presenceFrame(r.slots())
	for _, p := range r.peers {
		if p != except {
			p.send(outFrame{data: f})
		}
	}
}

func (s *Server) acquire(space []byte) *room {
	key := string(space)
	s.mu.Lock()
	defer s.mu.Unlock()
	r := s.rooms[key]
	if r == nil {
		r = &room{key: key, peers: map[string]*peer{}}
		s.rooms[key] = r
	}
	r.refs++
	return r
}

func (s *Server) release(r *room) {
	s.mu.Lock()
	defer s.mu.Unlock()
	r.refs--
	if r.refs == 0 && s.rooms[r.key] == r {
		delete(s.rooms, r.key)
	}
}

// goneLocked tells every device of r that its space was deleted, and takes the room out, so
// that a space made again with the same id gets a room of its own. Under r.mu.
func (s *Server) goneLocked(r *room) {
	r.gone = true
	for k, p := range r.peers {
		p.send(outFrame{data: goneFrame, last: true, code: statusNormal, reason: "gone"})
		delete(r.peers, k)
	}
	s.mu.Lock()
	if s.rooms[r.key] == r {
		delete(s.rooms, r.key)
	}
	s.mu.Unlock()
}

// deleteSpace deletes a space with everything of it; its devices online hear gone. It goes
// through the room, so that no hello can slip in between.
func (s *Server) deleteSpace(id []byte, reason string) (bool, error) {
	r := s.acquire(id)
	defer s.release(r)
	r.mu.Lock()
	defer r.mu.Unlock()
	found, err := s.st.DeleteSpace(s.ctx, id)
	if err != nil {
		return false, err
	}
	if found {
		s.m.deleted.Inc(reason)
	}
	s.goneLocked(r)
	return found, nil
}

// notifyGone tells the devices online of a space the sweep deleted.
func (s *Server) notifyGone(id []byte) {
	s.mu.Lock()
	r := s.rooms[string(id)]
	if r != nil {
		r.refs++
	}
	s.mu.Unlock()
	if r == nil {
		return
	}
	r.mu.Lock()
	if _, err := s.st.Space(s.ctx, id); errors.Is(err, store.ErrNotFound) {
		s.goneLocked(r)
	}
	r.mu.Unlock()
	s.release(r)
}

// takeBig takes one of the big frames' places; false when ctx ended first.
func (s *Server) takeBig(ctx context.Context) bool {
	select {
	case s.big <- struct{}{}:
		return true
	case <-ctx.Done():
		return false
	}
}

func (s *Server) giveBig() { <-s.big }

// admit takes a socket in, if the instance and the address have room for it.
func (s *Server) admit(key string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shut || s.conns >= s.lim.MaxConns || s.waiting[key] >= s.lim.MaxWaiting {
		return false
	}
	s.conns++
	s.waiting[key]++
	s.wg.Add(1)
	return true
}

// greeted: the socket of key no longer waits for its hello.
func (s *Server) greeted(key string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.waiting[key]--; s.waiting[key] <= 0 {
		delete(s.waiting, key)
	}
}

func (s *Server) ended() {
	s.mu.Lock()
	s.conns--
	s.mu.Unlock()
	s.wg.Done()
}

func (s *Server) isShut() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.shut
}

// serveWS is GET /sync/v1/ws: the socket of one device. The handler's goroutine reads it; a
// writer of its own writes it.
func (s *Server) serveWS(w http.ResponseWriter, r *http.Request) {
	key := clientKey(r)
	if !s.admit(key) {
		s.fail(w, http.StatusServiceUnavailable, "busy")
		return
	}
	defer s.ended()
	conn, err := websocket.Accept(w, r, &websocket.AcceptOptions{
		// serveHTTP checked the origin, exactly (Options.Origins); Accept's own check would
		// take every origin of the request's host.
		InsecureSkipVerify: true,
		CompressionMode:    websocket.CompressionDisabled, // sealed bytes do not compress
	})
	if err != nil {
		s.greeted(key)
		return // Accept answered
	}
	p := s.hello(conn)
	s.greeted(key)
	if p == nil {
		return
	}
	p.read()
}

// refuse answers a socket that gets no place: frame (nil: none), then the close.
func (s *Server) refuse(conn *websocket.Conn, frame []byte, code websocket.StatusCode, reason string) {
	if frame != nil {
		ctx, cancel := context.WithTimeout(s.ctx, s.lim.WriteTimeout)
		_ = conn.Write(ctx, websocket.MessageText, frame)
		cancel()
	}
	_ = conn.Close(code, reason)
}

// hello reads the first frame and gives the device its place: the welcome with what it lacks,
// presence to the others. nil when the socket gets none (it is closed then).
func (s *Server) hello(conn *websocket.Conn) *peer {
	conn.SetReadLimit(helloLimit)
	ctx, cancel := context.WithTimeout(s.ctx, s.lim.HelloTimeout)
	typ, data, err := conn.Read(ctx)
	late := ctx.Err() != nil && s.ctx.Err() == nil
	cancel()
	if err != nil {
		switch {
		case errors.Is(err, websocket.ErrMessageTooBig):
			s.m.rejected.Inc("too_big")
		case late:
			s.m.closes.Inc("timeout")
		default:
			s.m.closes.Inc("lost")
		}
		_ = conn.CloseNow()
		return nil
	}
	var f inFrame
	if typ != websocket.MessageText || json.Unmarshal(data, &f) != nil || f.T != "hello" ||
		len(f.Space) != spaceLen || len(f.Token) != tokenLen || len(f.Device) != deviceLen || f.Since < 0 {
		s.m.rejected.Inc("invalid")
		s.refuse(conn, nil, statusPolicy, "expected a hello")
		return nil
	}
	sp, err := s.st.Authenticate(s.ctx, f.Space, f.Token)
	if errors.Is(err, store.ErrUnauthorized) {
		s.m.rejected.Inc("unauthorized")
		s.refuse(conn, errorFrame("unauthorized", ""), statusPolicy, "unauthorized")
		return nil
	}
	if err != nil {
		s.storeFailed("hello", err)
		s.refuse(conn, nil, statusInternal, "internal")
		return nil
	}

	// A welcome with a large snapshot takes a big frame's place until it is written.
	release := func() {}
	if since := f.Since; sp.HasSnapshot && (since == 0 || since < sp.SnapUpto || since > sp.Head) && sp.Bytes-sp.TailBytes > bigFrame {
		if !s.takeBig(s.ctx) {
			_ = conn.CloseNow()
			return nil
		}
		release = sync.OnceFunc(s.giveBig)
	}

	r := s.acquire(f.Space)
	refuse := func(frame []byte, code websocket.StatusCode, reason string) *peer {
		r.mu.Unlock()
		release()
		s.release(r)
		s.refuse(conn, frame, code, reason)
		return nil
	}
	r.mu.Lock()
	switch {
	case s.isShut():
		return refuse(nil, statusGoingAway, "restart")
	case r.gone:
		return refuse(goneFrame, statusNormal, "gone")
	}
	dkey := string(f.Device)
	old := r.peers[dkey]
	if old == nil && len(r.peers) >= s.lim.MaxSockets {
		s.m.rejected.Inc("full")
		return refuse(errorFrame("full", ""), statusPolicy, "full")
	}
	now := s.now()
	slot, err := s.st.Join(s.ctx, f.Space, f.Device, now, s.lim.MaxDevices)
	switch {
	case errors.Is(err, store.ErrFull):
		s.m.rejected.Inc("full")
		return refuse(errorFrame("full", ""), statusPolicy, "full")
	case errors.Is(err, store.ErrNotFound): // deleted since Authenticate
		return refuse(goneFrame, statusNormal, "gone")
	case err != nil:
		s.storeFailed("join", err)
		return refuse(nil, statusInternal, "internal")
	}
	// Under the room's lock no entry can come between what the welcome holds and what the
	// device hears next.
	sp, err = s.st.Space(s.ctx, f.Space)
	if err != nil {
		s.storeFailed("hello", err)
		return refuse(nil, statusInternal, "internal")
	}
	since := f.Since
	if since > sp.Head { // Phloem lost entries (a restored backup): everything it has
		since = 0
	}
	var snap *store.Snapshot
	after := since
	if sp.HasSnapshot && (since == 0 || since < sp.SnapUpto) {
		got, err := s.st.Snapshot(s.ctx, f.Space)
		if err != nil {
			s.storeFailed("hello", err)
			return refuse(nil, statusInternal, "internal")
		}
		snap, after = &got, got.Upto
	}
	entries, err := s.st.EntriesAfter(s.ctx, f.Space, after)
	if err != nil {
		s.storeFailed("hello", err)
		return refuse(nil, statusInternal, "internal")
	}

	pctx, pcancel := context.WithCancel(s.ctx)
	p := &peer{
		s: s, conn: conn, room: r, ctx: pctx, cancel: pcancel,
		space: f.Space, device: f.Device, dkey: dkey, slot: slot,
		out: make(chan outFrame, s.lim.QueueFrames+2),
	}
	if old != nil {
		old.send(outFrame{last: true, code: statusReplaced, reason: "replaced"})
		s.m.closes.Inc("replaced")
	}
	r.peers[dkey] = p
	p.send(outFrame{data: welcomeFrame(slot, sp.Head, now.UnixMilli(), r.slots(), snap, entries), release: release, first: true})
	if old == nil {
		r.presence(p)
	}
	if sp.TailBytes > s.lim.SoftTail {
		p.asked = true
		p.send(outFrame{data: compactFrame})
	}
	r.mu.Unlock()
	conn.SetReadLimit(s.lim.readLimit())
	s.wg.Add(1)
	go p.write()
	return p
}

// peer is the socket of one device in its room.
type peer struct {
	s      *Server
	conn   *websocket.Conn
	room   *room
	ctx    context.Context // ends the socket: the writer stops and the reads fail
	cancel context.CancelFunc
	space  []byte
	device []byte
	dkey   string
	slot   int

	out     chan outFrame
	queued  atomic.Int64 // the bytes in out
	pinging atomic.Bool

	// Guarded by room.mu:
	ending bool // a last frame is queued, or the socket goes: nothing more is queued
	asked  bool // compact was asked of it since the last snapshot
}

// outFrame is one frame for a device, or with data nil only the close.
type outFrame struct {
	data    []byte
	release func() // once the frame is written or dropped; nil: nothing
	first   bool   // the welcome: not held to the queue's bounds
	last    bool   // the socket closes after it, with code and reason
	code    websocket.StatusCode
	reason  string
}

func (f outFrame) free() {
	if f.release != nil {
		f.release()
	}
}

// send queues a frame; a device too far behind is let go (it catches up when it comes back).
// Under room.mu.
func (p *peer) send(f outFrame) {
	if p.ending {
		f.free()
		return
	}
	if !f.first && !f.last && (len(p.out) >= p.s.lim.QueueFrames || p.queued.Load()+int64(len(f.data)) > p.s.lim.QueueBytes) {
		f.free()
		p.ending = true
		p.s.m.closes.Inc("slow")
		p.cancel()
		return
	}
	if f.last {
		p.ending = true
	}
	p.queued.Add(int64(len(f.data)))
	p.out <- f // never blocks: the capacity has room for the first and the last frame beyond QueueFrames
}

// answer queues a frame for this device alone.
func (p *peer) answer(frame []byte) {
	p.room.mu.Lock()
	p.send(outFrame{data: frame})
	p.room.mu.Unlock()
}

// endLocked queues the close: the frames before it are written first. Under room.mu.
func (p *peer) endLocked(code websocket.StatusCode, reason, metric string) {
	if !p.ending {
		p.s.m.closes.Inc(metric)
	}
	p.send(outFrame{last: true, code: code, reason: reason})
}

// write writes the queued frames in order, pings, and closes the socket after the last frame,
// or when the socket ends.
func (p *peer) write() {
	defer p.s.wg.Done()
	defer func() {
		p.cancel()
		_ = p.conn.CloseNow()
		p.room.mu.Lock()
		p.ending = true
		p.room.mu.Unlock()
		p.drain()
	}()
	ping := time.NewTicker(p.s.lim.PingInterval)
	defer ping.Stop()
	for {
		select {
		case f := <-p.out:
			if !p.deliver(f) {
				return
			}
		case <-ping.C:
			if p.pinging.CompareAndSwap(false, true) {
				p.s.wg.Add(1)
				go p.ping()
			}
		case <-p.ctx.Done():
			return
		}
	}
}

// deliver writes one frame; false when the socket is done.
func (p *peer) deliver(f outFrame) bool {
	p.queued.Add(-int64(len(f.data)))
	if f.data != nil {
		ctx, cancel := context.WithTimeout(p.ctx, p.s.lim.WriteTimeout)
		err := p.conn.Write(ctx, websocket.MessageText, f.data)
		cancel()
		f.free()
		if err != nil {
			return false
		}
	}
	if f.last {
		_ = p.conn.Close(f.code, f.reason) // the close handshake: at most some seconds
		return false
	}
	return true
}

func (p *peer) drain() {
	for {
		select {
		case f := <-p.out:
			p.queued.Add(-int64(len(f.data)))
			f.free()
		default:
			return
		}
	}
}

// ping: a device that does not answer within Limits.PingTimeout is gone.
func (p *peer) ping() {
	defer p.s.wg.Done()
	defer p.pinging.Store(false)
	ctx, cancel := context.WithTimeout(p.ctx, p.s.lim.PingTimeout)
	defer cancel()
	if err := p.conn.Ping(ctx); err != nil && p.ctx.Err() == nil {
		p.s.m.closes.Inc("timeout")
		p.cancel()
	}
}

// read handles the device's frames until the socket ends, then takes it out of its room.
func (p *peer) read() {
	defer p.exit()
	for {
		data, release, err := p.next()
		if err != nil {
			if errors.Is(err, websocket.ErrMessageTooBig) {
				p.s.m.rejected.Inc("too_big")
			}
			return
		}
		ok := p.handle(data)
		release()
		if !ok {
			return
		}
	}
}

var errBinary = errors.New("binary frame")

// next reads one frame: once begun, it has Limits.FrameTimeout to arrive, and beyond bigFrame
// it waits for a big frame's place (release gives it back).
func (p *peer) next() (data []byte, release func(), err error) {
	ctx, cancel := context.WithCancel(p.ctx)
	defer cancel()
	typ, r, err := p.conn.Reader(ctx)
	if err != nil {
		return nil, nil, err
	}
	timer := time.AfterFunc(p.s.lim.FrameTimeout, cancel)
	defer timer.Stop()
	if typ != websocket.MessageText {
		return nil, nil, errBinary
	}
	held := false
	release = func() {}
	buf := make([]byte, 0, 4096)
	for {
		if len(buf) == cap(buf) {
			if cap(buf) >= bigFrame && !held {
				if !p.s.takeBig(ctx) {
					return nil, nil, ctx.Err()
				}
				held, release = true, sync.OnceFunc(p.s.giveBig)
			}
			buf = slices.Grow(buf, cap(buf))
		}
		n, err := r.Read(buf[len(buf):cap(buf)])
		buf = buf[:len(buf)+n]
		if err == io.EOF {
			return buf, release, nil
		}
		if err != nil {
			release()
			return nil, nil, err
		}
	}
}

// exit takes the device out of its room when its socket ended: the others hear it went.
func (p *peer) exit() {
	r := p.room
	r.mu.Lock()
	if r.peers[p.dkey] == p {
		delete(r.peers, p.dkey)
		r.presence(nil)
	}
	ending := p.ending
	p.ending = true
	r.mu.Unlock()
	if !ending { // nothing more to say: the socket is lost or broke the protocol
		p.s.m.closes.Inc("lost")
		p.cancel()
	}
	p.s.release(r)
}

// handle answers one frame; false when the socket is to end (its close is queued).
func (p *peer) handle(data []byte) bool {
	var f inFrame
	if err := json.Unmarshal(data, &f); err != nil {
		return p.violation("invalid frame")
	}
	switch f.T {
	case "push":
		return p.push(&f)
	case "snapshot":
		return p.snapshot(&f)
	case "leave":
		return p.leaveSpace()
	case "delete":
		return p.deleteSpace()
	default: // a second hello among them
		return p.violation("unexpected frame")
	}
}

// violation ends a socket that broke the protocol (1008).
func (p *peer) violation(reason string) bool {
	p.s.m.rejected.Inc("invalid")
	p.room.mu.Lock()
	p.endLocked(statusPolicy, reason, "invalid")
	p.room.mu.Unlock()
	return false
}

// internalLocked ends a socket after a store call failed (1011). Under room.mu.
func (p *peer) internalLocked(op string, err error) bool {
	p.s.storeFailed(op, err)
	p.endLocked(statusInternal, "internal", "internal")
	return false
}

// push keeps a sealed batch as the space's next entry: stored to the device, entry to the
// others; a push the device sent again gets its entry's number.
func (p *peer) push(f *inFrame) bool {
	s := p.s
	if f.ID == "" || utf8.RuneCountInString(f.ID) > s.lim.MaxPushID || !utf8.ValidString(f.ID) {
		return p.violation("invalid push id")
	}
	if f.Data == nil {
		return p.violation("push without data")
	}
	if len(f.Data) > s.lim.MaxEntry {
		s.m.rejected.Inc("too_big")
		p.answer(errorFrame("too_big", f.ID))
		return true
	}
	now := s.now()
	if !s.pushes.take(string(p.space), 1, now) {
		s.m.rejected.Inc("slow_down")
		p.answer(errorFrame("slow_down", f.ID))
		return true
	}
	r := p.room
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.gone || p.ending {
		return false
	}
	ap, err := s.st.Append(s.ctx, p.space, p.slot, f.ID, f.Data, now, s.lim.HardTail)
	switch {
	case errors.Is(err, store.ErrCompactFirst):
		s.m.rejected.Inc("compact_first")
		p.send(outFrame{data: errorFrame("compact_first", f.ID)})
		return true
	case errors.Is(err, store.ErrNotFound):
		s.goneLocked(r)
		return false
	case err != nil:
		return p.internalLocked("push", err)
	}
	p.send(outFrame{data: storedFrame(f.ID, ap.Nr)})
	if ap.Again {
		s.m.again.Inc()
	} else {
		s.m.entries.Inc()
		s.m.entryBytes.Add(float64(len(f.Data)))
		e := entryFrame(ap.Nr, p.slot, f.Data)
		for _, q := range r.peers {
			if q != p {
				q.send(outFrame{data: e})
			}
		}
	}
	if ap.TailBytes > s.lim.SoftTail && !p.asked {
		p.asked = true
		p.send(outFrame{data: compactFrame})
	}
	return true
}

// snapshot keeps the space's snapshot as of entry upto; the entries up to it go. One older
// than the kept one is not kept: the answer names the kept one's upto.
func (p *peer) snapshot(f *inFrame) bool {
	s := p.s
	if f.Upto == nil || *f.Upto < 0 || f.Data == nil {
		return p.violation("invalid snapshot")
	}
	if len(f.Data) > s.lim.MaxSnapshot {
		s.m.rejected.Inc("too_big")
		p.answer(errorFrame("too_big", ""))
		return true
	}
	now := s.now()
	if !s.pushes.take(string(p.space), float64(1+len(f.Data)/snapshotUnit), now) {
		s.m.rejected.Inc("slow_down")
		p.answer(errorFrame("slow_down", ""))
		return true
	}
	r := p.room
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.gone || p.ending {
		return false
	}
	err := s.st.PutSnapshot(s.ctx, p.space, *f.Upto, f.Data, now)
	switch {
	case errors.Is(err, store.ErrStale):
		sp, err := s.st.Space(s.ctx, p.space)
		if err != nil {
			return p.internalLocked("snapshot", err)
		}
		p.send(outFrame{data: snapshottedFrame(sp.SnapUpto)})
		return true
	case errors.Is(err, store.ErrAhead):
		s.m.rejected.Inc("invalid")
		p.endLocked(statusPolicy, "snapshot beyond the head", "invalid")
		return false
	case errors.Is(err, store.ErrNotFound):
		s.goneLocked(r)
		return false
	case err != nil:
		return p.internalLocked("snapshot", err)
	}
	s.m.snapshots.Inc()
	s.m.snapBytes.Add(float64(len(f.Data)))
	p.send(outFrame{data: snapshottedFrame(*f.Upto)})
	for _, q := range r.peers {
		q.asked = false
	}
	return true
}

// leaveSpace frees the device's slot („Nur dieses Gerät trennen") and closes its socket (1000).
func (p *peer) leaveSpace() bool {
	r := p.room
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.gone || p.ending {
		return false
	}
	if err := p.s.st.Leave(p.s.ctx, p.space, p.device); err != nil {
		return p.internalLocked("leave", err)
	}
	if r.peers[p.dkey] == p {
		delete(r.peers, p.dkey)
	}
	p.endLocked(statusNormal, "left", "left")
	r.presence(nil)
	return false
}

// deleteSpace is DELETE /sync/v1/space from a socket: every device online, this one too,
// hears gone.
func (p *peer) deleteSpace() bool {
	if _, err := p.s.deleteSpace(p.space, "deleted"); err != nil {
		p.room.mu.Lock()
		defer p.room.mu.Unlock()
		return p.internalLocked("delete", err)
	}
	return false
}
