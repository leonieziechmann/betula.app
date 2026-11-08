package server

import (
	"bytes"
	"errors"
	"math"
	"net/http"
	"strconv"
	"strings"
	"sync"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

// putPart is PUT /sync/v1/part with Betula-Part: a part of a shared calendar, made or replaced;
// the sockets that watch it hear its version. 200 {"rev"}; 409 exists (another space's part, or
// one that is gone: a part never comes back); 413 too_big; 429 slow_down (the space's pushes, or
// its new parts of the day: Limits.PartsPerDay), too_many (Limits.MaxParts); 507 budget (a new
// part; a replacement goes on).
func (s *Server) putPart(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	id, ok := decode64(r.Header.Get("Betula-Part"), partLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	var body struct {
		Data b64 `json:"data"`
	}
	if !s.readBody(w, r, int64(b64len(s.lim.MaxPart))+1<<10, &body) {
		return
	}
	if body.Data == nil {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	if len(body.Data) > s.lim.MaxPart {
		s.fail(w, http.StatusRequestEntityTooLarge, "too_big")
		return
	}
	now, key := s.now(), string(sp.ID)
	if !s.pushes.take(key, 1, now) {
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	// Whose the id is comes from a reader: a try at another space's part, or at one that is
	// gone, costs no write.
	owner, err := s.st.PartOwner(r.Context(), id)
	made := false // a new part's token is taken
	switch {
	case errors.Is(err, store.ErrGone):
		s.fail(w, http.StatusConflict, "exists")
		return
	case errors.Is(err, store.ErrNotFound):
		if s.overBudget() {
			s.fail(w, http.StatusInsufficientStorage, "budget")
			return
		}
		// A part deleted stays gone for Limits.PartGoneFor, whatever becomes of its space: the
		// new parts of a day bound what a space can leave behind.
		if !s.newParts.take(key, 1, now) {
			s.fail(w, http.StatusTooManyRequests, "slow_down")
			return
		}
		made = true
	case err != nil:
		s.internal(w, "part", err)
		return
	case !bytes.Equal(owner, sp.ID):
		s.fail(w, http.StatusConflict, "exists")
		return
	}
	rev, err := s.st.PutPart(r.Context(), id, sp.ID, body.Data, now, s.lim.MaxParts)
	if made && (err != nil || rev != 1) { // no part was made by this request
		s.newParts.give(key, 1)
	}
	switch {
	case errors.Is(err, store.ErrExists):
		s.fail(w, http.StatusConflict, "exists")
	case errors.Is(err, store.ErrTooMany):
		s.fail(w, http.StatusTooManyRequests, "too_many")
	case errors.Is(err, store.ErrNotFound):
		s.fail(w, http.StatusUnauthorized, "unauthorized") // deleted meanwhile
	case err != nil:
		s.internal(w, "part", err)
	default:
		s.m.parts.Inc("put")
		s.tellPart(id, rev)
		writeJSON(w, http.StatusOK, map[string]int64{"rev": rev})
	}
}

// deletePart is DELETE /sync/v1/part with Betula-Part („Nicht mehr teilen"): the space's part is
// gone from now on, and the sockets that watch it hear so. 204, also for an id that is not a
// part of the space (nothing changes then, and nothing is written).
func (s *Server) deletePart(w http.ResponseWriter, r *http.Request) {
	sp, ok := s.authenticate(w, r)
	if !ok {
		return
	}
	id, ok := decode64(r.Header.Get("Betula-Part"), partLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	owner, err := s.st.PartOwner(r.Context(), id)
	switch {
	case errors.Is(err, store.ErrGone), errors.Is(err, store.ErrNotFound):
	case err != nil:
		s.internal(w, "part", err)
		return
	case bytes.Equal(owner, sp.ID):
		found, err := s.st.DeletePart(r.Context(), id, sp.ID, s.now())
		if err != nil {
			s.internal(w, "part", err)
			return
		}
		if found {
			s.m.parts.Inc("deleted")
			s.tellPart(id, revGone)
		}
	}
	w.WriteHeader(http.StatusNoContent)
}

// getPart is GET /sync/v1/part with Betula-Part: the part, for anyone with its id (it is sealed
// with a key Phloem never sees). 200 {"data", "rev"} with ETag "<rev>"; 304 when If-None-Match
// names that version; 410 gone; 404 not_found; 429 slow_down (Limits.PartGetsPerAddr, found or
// not). It costs a read, never the writer.
func (s *Server) getPart(w http.ResponseWriter, r *http.Request) {
	id, ok := decode64(r.Header.Get("Betula-Part"), partLen)
	if !ok {
		s.fail(w, http.StatusBadRequest, "invalid")
		return
	}
	if !s.partGets.take(clientKey(r), 1, s.now()) {
		s.fail(w, http.StatusTooManyRequests, "slow_down")
		return
	}
	p, err := s.st.Part(r.Context(), id)
	switch {
	case errors.Is(err, store.ErrGone):
		s.m.parts.Inc("gone")
		writeError(w, http.StatusGone, "gone")
		return
	case errors.Is(err, store.ErrNotFound):
		s.m.parts.Inc("missed")
		writeError(w, http.StatusNotFound, "not_found")
		return
	case err != nil:
		s.internal(w, "part", err)
		return
	}
	etag := `"` + strconv.FormatInt(p.Rev, 10) + `"`
	w.Header().Set("ETag", etag)
	if noneMatch(r.Header.Values("If-None-Match"), etag) {
		s.m.parts.Inc("not_modified")
		w.WriteHeader(http.StatusNotModified)
		return
	}
	s.m.parts.Inc("served")
	writeJSON(w, http.StatusOK, struct {
		Data b64   `json:"data"`
		Rev  int64 `json:"rev"`
	}{p.Data, p.Rev})
}

// noneMatch says whether the If-None-Match headers name etag, or any version (*), by the weak
// comparison RFC 9110 (13.1.2) asks for.
func noneMatch(headers []string, etag string) bool {
	for _, h := range headers {
		for _, tag := range strings.Split(h, ",") {
			tag = strings.TrimSpace(tag)
			if tag == "*" || strings.TrimPrefix(tag, "W/") == etag {
				return true
			}
		}
	}
	return false
}

// revGone is the version of a part that is gone, in what a socket was told: above every other,
// so that nothing of the part is told after it.
const revGone = math.MaxInt64

// watchers are the sockets that follow parts, by part id (docs/phloem/protocol.md section 4,
// watch), of any space: in memory, as the sockets are. mu guards the map alone and is never held
// while another lock is taken: the frames go out afterwards, under each peer's room.mu, one room
// at a time.
//
// What a socket hears of a part is in order however the writers' frames cross: each peer keeps
// the version it was told last of every part it watches (peer.revs), and hears only a newer one.
// A watch puts its parts in revs, then in the map, then reads them from the store, and answers
// with what it read or what the socket was told meanwhile, whichever is newer: a write after the
// read finds the socket in the map, one before it is in the read, so no version is missed, and
// no watching is older than a part frame before it.
type watchers struct {
	mu    sync.Mutex
	parts map[string]map[*peer]struct{}
}

// set makes ids the parts p watches, in place of those before; nil: none.
func (w *watchers) set(p *peer, ids [][]byte) {
	w.mu.Lock()
	defer w.mu.Unlock()
	for _, id := range p.watching {
		set := w.parts[id]
		delete(set, p)
		if len(set) == 0 {
			delete(w.parts, id)
		}
	}
	p.watching = p.watching[:0]
	for _, raw := range ids {
		id := string(raw)
		set := w.parts[id]
		if set == nil {
			set = map[*peer]struct{}{}
			w.parts[id] = set
		}
		if _, twice := set[p]; !twice {
			set[p] = struct{}{}
			p.watching = append(p.watching, id)
		}
	}
}

// of are the sockets that watch the part id.
func (w *watchers) of(id []byte) []*peer {
	w.mu.Lock()
	defer w.mu.Unlock()
	set := w.parts[string(id)]
	out := make([]*peer, 0, len(set))
	for p := range set {
		out = append(out, p)
	}
	return out
}

// size is the number of parts that sockets watch.
func (w *watchers) size() int {
	w.mu.Lock()
	defer w.mu.Unlock()
	return len(w.parts)
}

// tellPart tells the sockets that watch the part id its version rev, or with revGone that it is
// gone; a socket that was told as much or more already hears nothing. Under no lock.
func (s *Server) tellPart(id []byte, rev int64) {
	frame := partFrame(id, rev)
	if rev == revGone {
		frame = partGoneFrame(id)
	}
	key := string(id)
	for _, p := range s.watchers.of(id) {
		p.room.mu.Lock()
		if told, ok := p.revs[key]; ok && rev > told {
			p.revs[key] = rev
			p.send(outFrame{data: frame})
		}
		p.room.mu.Unlock()
	}
}

// watchParts makes ids the parts p follows, in place of those before, and answers with the
// version of each now (docs at watchers). Under no lock.
func (s *Server) watchParts(p *peer, ids [][]byte) error {
	r := p.room
	r.mu.Lock()
	revs := make(map[string]int64, len(ids))
	for _, id := range ids {
		revs[string(id)] = p.revs[string(id)] // what it was told of a part it goes on watching
	}
	p.revs = revs
	r.mu.Unlock()
	s.watchers.set(p, ids)
	states, err := s.st.PartStates(s.ctx, ids)
	if err != nil {
		return err
	}
	now := make([]int64, len(ids))
	r.mu.Lock()
	defer r.mu.Unlock()
	for i, id := range ids {
		rev := states[i].Rev
		if states[i].Gone {
			rev = revGone
		}
		rev = max(rev, p.revs[string(id)])
		p.revs[string(id)] = rev
		now[i] = rev
	}
	p.send(outFrame{data: watchingFrame(ids, now)})
	return nil
}

// unwatch forgets the parts p watched: its socket ended.
func (s *Server) unwatch(p *peer) {
	s.watchers.set(p, nil)
}

// partsGone tells the watchers of the parts that went with their space that they are gone. Not
// under that space's room lock: telling takes the room lock of every watcher, and a device of
// the space may watch its own part.
func (s *Server) partsGone(ids [][]byte) {
	if len(ids) == 0 {
		return
	}
	s.m.parts.Add(float64(len(ids)), "deleted")
	for _, id := range ids {
		s.tellPart(id, revGone)
	}
}

// watch follows the parts of shared calendars the device names, at most Limits.MaxWatch, in
// place of those before; the answer is the version of each now. It counts as a push of the
// space. Too many ids, or one that is not a part's, break the protocol (1008).
func (p *peer) watch(f *inFrame) bool {
	s := p.s
	if f.Parts == nil || len(f.Parts) > s.lim.MaxWatch {
		return p.violation("invalid watch")
	}
	ids := make([][]byte, len(f.Parts))
	for i, id := range f.Parts {
		if len(id) != partLen {
			return p.violation("invalid part id")
		}
		ids[i] = id
	}
	if !s.pushes.take(string(p.space), 1, s.now()) {
		s.m.rejected.Inc("slow_down")
		p.answer(errorFrame("slow_down", ""))
		return true
	}
	if err := s.watchParts(p, ids); err != nil {
		p.room.mu.Lock()
		defer p.room.mu.Unlock()
		return p.internalLocked("watch", err)
	}
	return true
}
