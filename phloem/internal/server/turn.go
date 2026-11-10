package server

import (
	"context"
	"crypto/rand"
	"errors"
	"fmt"
	"log/slog"
	"math/big"
	"net"
	"net/netip"
	"slices"
	"strconv"
	"sync"
	"time"

	"github.com/pion/logging"
	"github.com/pion/turn/v5"

	"github.com/leonieziechmann/betula/phloem/internal/oplog"
)

// TURN (step 2, docs/phloem/protocol.md section 6): STUN and TURN for the devices of a space, so
// that two devices that cannot reach each other directly still find a way between them. Pion
// relays; Phloem decides who may use it, and to where:
//
//   - a credential per socket (the frame turn), good while that socket lives;
//   - relays over UDP only, from TURN's port range, each at most Limits.RelayRate a second each
//     way, all of them at most Limits.RelayTotal;
//   - a peer only at the address another socket of the same space came from (Traefik's
//     X-Real-Ip), or that another socket's credential allocated from (a device behind a VPN,
//     or whose socket goes over IPv6 and its UDP over IPv4), never at a special-purpose address
//     nor at Phloem's own (permit). An allocation over UDP comes only from an address that got
//     Phloem's nonce (nonceConn). Phloem's firewall holds the same line once more
//     (phloem/deploy/turn.nft).
type TURN struct {
	// Host is the site's host, the name the URLs give for STUN and TURN over UDP and TCP.
	Host string
	// Port is where STUN and TURN listen, over UDP and over TCP, and the URLs' port.
	Port int
	// IP is the host's public IPv4 address: the address of the relays, and never a peer.
	IP netip.Addr
	// RelayMin and RelayMax are the UDP ports of the relays, both included.
	RelayMin, RelayMax int
	// TLSHost names turns:<TLSHost>:443?transport=tcp: Traefik ends TLS for it and hands the
	// TCP stream to Port. Empty: no such URL.
	TLSHost string
	// PacketConn and Listener are where STUN and TURN are served (serve opens them on Port).
	PacketConn net.PacketConn
	Listener   net.Listener
	// RelayBind is the local address a relay binds to ("0.0.0.0"; the tests 127.0.0.1).
	RelayBind string

	// loopback lets 127.0.0.0/8 count as public: the tests alone, which run on loopback.
	loopback bool
}

// enabled says whether TURN is configured at all.
func (t *TURN) enabled() bool { return t != nil && t.PacketConn != nil }

func (t *TURN) validate() error {
	switch {
	case t.Host == "" || t.Port < 1 || t.Port > 65535:
		return errors.New("TURN needs a host and a port")
	case !t.IP.Is4():
		return errors.New("TURN needs the host's public IPv4 address")
	case t.RelayMin < 1024 || t.RelayMax > 65535 || t.RelayMin > t.RelayMax:
		return fmt.Errorf("invalid relay ports %d-%d", t.RelayMin, t.RelayMax)
	case t.Listener == nil:
		return errors.New("TURN needs a TCP listener beside the UDP one")
	case !t.loopback && !public(t.IP):
		return fmt.Errorf("TURN's address %s is not public", t.IP)
	}
	return nil
}

// urls are the ICE servers a device is given: STUN first, then TURN over UDP, TCP and TLS.
func (t *TURN) urls() []string {
	hp := net.JoinHostPort(t.Host, strconv.Itoa(t.Port))
	out := []string{"stun:" + hp, "turn:" + hp + "?transport=udp", "turn:" + hp + "?transport=tcp"}
	if t.TLSHost != "" {
		out = append(out, "turns:"+t.TLSHost+":443?transport=tcp")
	}
	return out
}

// special are the IPv4 ranges that are no address on the internet (RFC 6890 and its updates):
// never a peer, whoever asks.
var special = []netip.Prefix{
	netip.MustParsePrefix("0.0.0.0/8"),       // this network
	netip.MustParsePrefix("10.0.0.0/8"),      // private
	netip.MustParsePrefix("100.64.0.0/10"),   // shared address space (CGNAT)
	netip.MustParsePrefix("127.0.0.0/8"),     // loopback
	netip.MustParsePrefix("169.254.0.0/16"),  // link local
	netip.MustParsePrefix("172.16.0.0/12"),   // private
	netip.MustParsePrefix("192.0.0.0/24"),    // IETF protocol assignments
	netip.MustParsePrefix("192.0.2.0/24"),    // documentation
	netip.MustParsePrefix("192.31.196.0/24"), // AS112
	netip.MustParsePrefix("192.52.193.0/24"), // AMT
	netip.MustParsePrefix("192.88.99.0/24"),  // 6to4 relay anycast
	netip.MustParsePrefix("192.168.0.0/16"),  // private
	netip.MustParsePrefix("192.175.48.0/24"), // AS112
	netip.MustParsePrefix("198.18.0.0/15"),   // benchmarking
	netip.MustParsePrefix("198.51.100.0/24"), // documentation
	netip.MustParsePrefix("203.0.113.0/24"),  // documentation
	netip.MustParsePrefix("224.0.0.0/4"),     // multicast
	netip.MustParsePrefix("240.0.0.0/4"),     // reserved, and the broadcast address
}

// public says whether ip is an IPv4 address on the internet.
func public(ip netip.Addr) bool {
	ip = ip.Unmap()
	if !ip.Is4() {
		return false
	}
	for _, p := range special {
		if p.Contains(ip) {
			return false
		}
	}
	return true
}

// turnUser is the credential of one socket.
type turnUser struct {
	name, password string
	key            []byte // the long-term key, md5(name:realm:password)
	space          string // the space of its socket (the room's key)
	peer           *peer  // the socket it belongs to; nil once that socket ended
	allocs         int    // allocations made with it, now
	// from are the public addresses its allocations came from other than its socket's own,
	// newest last, at most Limits.AllocsPerSocket: peers for the other devices of the space
	// while its socket lives, also after an allocation ended (their permissions are renewed
	// every five minutes, and a device may let its allocation go once a way was found).
	from []netip.Addr
}

// turnServer is Pion's TURN with Phloem's rules around it.
type turnServer struct {
	s     *Server
	opt   TURN
	realm string
	srv   *turn.Server
	total *byteBucket // the bytes of all relays, each way: one bucket per way
	back  *byteBucket

	nonceKey []byte // binds the nonces handed out over UDP to their address (nonceConn)

	mu      sync.Mutex
	users   map[string]*turnUser              // by name
	members map[string]map[*turnUser]struct{} // the credentials of the sockets that live, by space
	clients map[string]*turnUser              // by the client address of an allocation (its 5-tuple's client side)
	spaces  map[string]int                    // allocations by space
	allocs  int                               // allocations in all
}

// newTURN starts Pion on the listeners of opt.
func newTURN(s *Server, opt TURN) (*turnServer, error) {
	if err := opt.validate(); err != nil {
		return nil, err
	}
	if opt.RelayBind == "" {
		opt.RelayBind = "0.0.0.0"
	}
	t := &turnServer{
		s: s, opt: opt, realm: opt.Host,
		total:    newByteBucket(float64(s.lim.RelayTotal), float64(s.lim.RelayTotal)),
		back:     newByteBucket(float64(s.lim.RelayTotal), float64(s.lim.RelayTotal)),
		nonceKey: randomBytes(32),
		users:    map[string]*turnUser{},
		members:  map[string]map[*turnUser]struct{}{},
		clients:  map[string]*turnUser{},
		spaces:   map[string]int{},
	}
	relays := &relays{t: t}
	srv, err := turn.NewServer(turn.ServerConfig{
		Realm:         t.realm,
		AuthHandler:   t.auth,
		QuotaHandler:  t.quota,
		LoggerFactory: pionLogs{},
		EventHandler: turn.EventHandler{
			OnAuth:              t.authed,
			OnAllocationCreated: t.allocated,
			OnAllocationDeleted: t.released,
			OnPermissionCreated: func(net.Addr, net.Addr, string, string, string, net.Addr, net.IP) { s.m.turn.Inc("permitted") },
		},
		PacketConnConfigs: []turn.PacketConnConfig{{PacketConn: &nonceConn{PacketConn: opt.PacketConn, t: t}, RelayAddressGenerator: relays, PermissionHandler: t.permit}},
		ListenerConfigs:   []turn.ListenerConfig{{Listener: opt.Listener, RelayAddressGenerator: relays, PermissionHandler: t.permit}},
	})
	if err != nil {
		return nil, err
	}
	t.srv = srv
	return t, nil
}

func (t *turnServer) close() error {
	return t.srv.Close()
}

// credential is the socket's credential, made on its first turn frame.
func (t *turnServer) credential(p *peer) *turnUser {
	t.mu.Lock()
	defer t.mu.Unlock()
	if p.turnUser != nil {
		return p.turnUser
	}
	u := &turnUser{name: token(16), password: token(16), space: p.room.key, peer: p}
	u.key = turn.GenerateAuthKey(u.name, t.realm, u.password)
	t.users[u.name] = u
	if t.members[u.space] == nil {
		t.members[u.space] = map[*turnUser]struct{}{}
	}
	t.members[u.space][u] = struct{}{}
	p.turnUser = u
	return u
}

// forget ends the credential of a socket that ended: Pion takes it no more, so its allocations
// end with their lifetime at the latest, and they get no new permission; the addresses they
// came from are peers no more.
func (t *turnServer) forget(p *peer) {
	t.mu.Lock()
	defer t.mu.Unlock()
	if u := p.turnUser; u != nil {
		delete(t.users, u.name)
		if m := t.members[u.space]; m != nil {
			delete(m, u)
			if len(m) == 0 {
				delete(t.members, u.space)
			}
		}
		u.peer = nil
		u.from = nil
	}
}

// auth is Pion's AuthHandler: the key of a credential whose socket lives.
func (t *turnServer) auth(ra *turn.RequestAttributes) (string, []byte, bool) {
	t.mu.Lock()
	defer t.mu.Unlock()
	u := t.users[ra.Username]
	if u == nil || u.peer == nil || ra.Realm != t.realm {
		return "", nil, false
	}
	return u.name, u.key, true
}

// key is the long-term key of the credential name whose socket lives; nil for none.
func (t *turnServer) key(name string) []byte {
	t.mu.Lock()
	defer t.mu.Unlock()
	if u := t.users[name]; u != nil && u.peer != nil {
		return u.key
	}
	return nil
}

// authed counts what Pion refused for want of a credential.
func (t *turnServer) authed(_, _ net.Addr, _, _, _, _ string, verdict bool) {
	if !verdict {
		t.s.m.turn.Inc("unauthorized")
	}
}

// quota is Pion's QuotaHandler: no new allocation beyond the socket's, the space's or all.
func (t *turnServer) quota(name, _ string, _ net.Addr) bool {
	t.mu.Lock()
	defer t.mu.Unlock()
	u := t.users[name]
	ok := u != nil && u.peer != nil && u.allocs < t.s.lim.AllocsPerSocket &&
		t.spaces[u.space] < t.s.lim.AllocsPerSpace && t.allocs < t.opt.RelayMax-t.opt.RelayMin+1
	if !ok {
		t.s.m.turn.Inc("quota")
	}
	return ok
}

// allocated keeps whose an allocation is, by its client address, for permit, and that address
// as one of its socket's device: a public address other than Phloem's own, which answered
// Phloem with the space's credential (over UDP its nonce, nonceConn; over TCP the handshake).
// Over TLS the allocation comes from Traefik, a private address, and adds nothing.
func (t *turnServer) allocated(src, _ net.Addr, _, name, _ string, _ net.Addr, _ int) {
	t.mu.Lock()
	defer t.mu.Unlock()
	u := t.users[name]
	if u == nil || u.peer == nil {
		return // its socket ended meanwhile: permit finds no owner and refuses every peer
	}
	t.clients[src.String()] = u
	u.allocs++
	t.spaces[u.space]++
	t.allocs++
	t.s.m.turn.Inc("allocated")
	if ip, ok := addrIP(src); ok && t.allowed(ip) && ip != t.opt.IP && ip != u.peer.addr && !slices.Contains(u.from, ip) {
		if len(u.from) >= t.s.lim.AllocsPerSocket {
			u.from = slices.Delete(u.from, 0, 1)
		}
		u.from = append(u.from, ip)
		t.s.m.turn.Inc("learned")
	}
}

func (t *turnServer) released(src, _ net.Addr, _, _, _ string) {
	t.mu.Lock()
	defer t.mu.Unlock()
	u := t.clients[src.String()]
	if u == nil {
		return
	}
	delete(t.clients, src.String())
	u.allocs--
	t.allocs--
	if t.spaces[u.space]--; t.spaces[u.space] <= 0 {
		delete(t.spaces, u.space)
	}
}

// permit is Pion's PermissionHandler, asked for every permission and channel binding (and
// asked again when the device renews one): a peer only at a public address another socket of
// the allocation's space came from or allocated from, never at Phloem's own.
func (t *turnServer) permit(client net.Addr, peerIP net.IP) bool {
	ip, ok := netip.AddrFromSlice(peerIP)
	if !ok {
		t.s.m.turn.Inc("refused")
		return false
	}
	ip = ip.Unmap()
	if !t.allowed(ip) || ip == t.opt.IP {
		t.s.m.turn.Inc("refused")
		return false
	}
	t.mu.Lock()
	u := t.clients[client.String()]
	var p *peer
	var allocatedFrom []*peer // the other sockets of the space whose allocations came from ip
	if u != nil && u.peer != nil {
		p = u.peer
		for o := range t.members[u.space] {
			if o != u && o.peer != nil && slices.Contains(o.from, ip) {
				allocatedFrom = append(allocatedFrom, o.peer)
			}
		}
	}
	t.mu.Unlock()
	if p == nil || !t.s.cameFrom(p, ip, allocatedFrom) {
		t.s.m.turn.Inc("refused")
		return false
	}
	return true
}

// allowed says whether ip may be a peer at all.
func (t *turnServer) allowed(ip netip.Addr) bool {
	if t.opt.loopback && ip.Is4() && ip.IsLoopback() {
		return true
	}
	return public(ip)
}

// cameFrom says whether another socket of p's space came from ip, or is one of allocatedFrom
// (the sockets whose allocations came from ip), while p is in its room.
func (s *Server) cameFrom(p *peer, ip netip.Addr, allocatedFrom []*peer) bool {
	r := p.room
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.gone || r.peers[p.dkey] != p {
		return false
	}
	for _, q := range r.peers {
		if q != p && (q.addr == ip || slices.Contains(allocatedFrom, q)) {
			return true
		}
	}
	return false
}

// token is n random bytes in base64url.
func token(n int) string {
	b := make([]byte, n)
	_, _ = rand.Read(b)
	return enc.EncodeToString(b)
}

// ---- relays ----

var errNoTCPRelays = errors.New("TCP relays are not offered")

// relays is the RelayAddressGenerator: a UDP port of TURN's range on RelayBind, announced at
// TURN's IP, its datagrams counted and held to the rates. No TCP relays (RFC 6062): WebRTC needs
// none.
type relays struct {
	t *turnServer
}

func (g *relays) Validate() error { return nil }

func (g *relays) AllocatePacketConn(conf turn.AllocateListenerConfig) (net.PacketConn, net.Addr, error) {
	opt := g.t.opt
	if conf.Network != "udp4" && conf.Network != "udp" {
		return nil, nil, fmt.Errorf("relays are IPv4 only, not %s", conf.Network)
	}
	listen := func(port int) (net.PacketConn, net.Addr, error) {
		conn, err := net.ListenPacket("udp4", net.JoinHostPort(opt.RelayBind, strconv.Itoa(port)))
		if err != nil {
			return nil, nil, err
		}
		lim := g.t.s.lim
		return &relay{PacketConn: conn, t: g.t, out: newByteBucket(float64(lim.RelayRate), float64(lim.RelayRate)),
				in: newByteBucket(float64(lim.RelayRate), float64(lim.RelayRate))},
			&net.UDPAddr{IP: opt.IP.AsSlice(), Port: port}, nil
	}
	if conf.RequestedPort != 0 { // EVEN-PORT or a reservation: within the range only
		if conf.RequestedPort < opt.RelayMin || conf.RequestedPort > opt.RelayMax {
			return nil, nil, fmt.Errorf("port %d is outside the relays' range", conf.RequestedPort)
		}
		return listen(conf.RequestedPort)
	}
	n := opt.RelayMax - opt.RelayMin + 1
	start, _ := rand.Int(rand.Reader, big.NewInt(int64(n)))
	for i := range n {
		port := opt.RelayMin + (int(start.Int64())+i)%n
		if conn, addr, err := listen(port); err == nil {
			return conn, addr, nil
		}
	}
	g.t.s.m.turn.Inc("no_port")
	return nil, nil, errors.New("every relay port is taken")
}

func (g *relays) AllocateListener(turn.AllocateListenerConfig) (net.Listener, net.Addr, error) {
	return nil, nil, errNoTCPRelays
}

func (g *relays) AllocateConn(turn.AllocateConnConfig) (net.Conn, error) {
	return nil, errNoTCPRelays
}

// relay is the socket of one allocation: datagrams beyond its rates, or beyond all relays',
// are dropped (counted); one to an address that may never be a peer is dropped, counted and
// logged, since Pion lets out only what permit allowed.
type relay struct {
	net.PacketConn
	t       *turnServer
	in, out *byteBucket
}

func (r *relay) WriteTo(b []byte, addr net.Addr) (int, error) {
	m := r.t.s.m
	ua, ok := addr.(*net.UDPAddr)
	ip, valid := netip.AddrFromSlice(func() []byte {
		if ok {
			return ua.IP
		}
		return nil
	}())
	if !ok || !valid || !r.t.allowed(ip.Unmap()) || ip.Unmap() == r.t.opt.IP {
		m.turn.Inc("forbidden")
		oplog.For("turn").Warn("a relay was to send to an address that is never a peer", "event", "turn.forbidden", "addr", addr.String())
		return len(b), nil
	}
	now := time.Now()
	if !r.out.take(float64(len(b)), now) || !r.t.total.take(float64(len(b)), now) {
		m.relayDropped.Add(float64(len(b)), "out")
		return len(b), nil
	}
	n, err := r.PacketConn.WriteTo(b, addr)
	m.relayed.Add(float64(n), "out")
	return n, err
}

func (r *relay) ReadFrom(b []byte) (int, net.Addr, error) {
	m := r.t.s.m
	for {
		n, addr, err := r.PacketConn.ReadFrom(b)
		if err != nil {
			return n, addr, err
		}
		now := time.Now()
		if r.in.take(float64(n), now) && r.t.back.take(float64(n), now) {
			m.relayed.Add(float64(n), "in")
			return n, addr, nil
		}
		m.relayDropped.Add(float64(n), "in")
	}
}

// byteBucket is a token bucket of bytes, safe for concurrent use.
type byteBucket struct {
	mu          sync.Mutex
	rate, burst float64
	have        float64
	at          time.Time
}

func newByteBucket(rate, burst float64) *byteBucket {
	return &byteBucket{rate: rate, burst: burst, have: burst}
}

func (b *byteBucket) take(n float64, now time.Time) bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	if !b.at.IsZero() {
		b.have = min(b.burst, b.have+now.Sub(b.at).Seconds()*b.rate)
	}
	b.at = now
	if b.have < n {
		return false
	}
	b.have -= n
	return true
}

// ---- Pion's log ----

// pionLogs hands Pion's log to slog at debug: what Pion says is about datagrams of strangers
// (scanners, garbage), which no alert should count; Phloem logs what it decides itself.
type pionLogs struct{}

func (pionLogs) NewLogger(scope string) logging.LeveledLogger { return pionLog{scope: scope} }

type pionLog struct{ scope string }

func (l pionLog) log(level slog.Level, msg string) {
	oplog.For("turn").Log(context.Background(), level, msg, "event", "turn.pion", "scope", l.scope)
}

func (l pionLog) Trace(msg string)      {}
func (l pionLog) Tracef(string, ...any) {}
func (l pionLog) Debug(msg string)      { l.log(slog.LevelDebug, msg) }
func (l pionLog) Debugf(format string, args ...any) {
	l.log(slog.LevelDebug, fmt.Sprintf(format, args...))
}
func (l pionLog) Info(msg string) { l.log(slog.LevelDebug, msg) }
func (l pionLog) Infof(format string, args ...any) {
	l.log(slog.LevelDebug, fmt.Sprintf(format, args...))
}
func (l pionLog) Warn(msg string) { l.log(slog.LevelDebug, msg) }
func (l pionLog) Warnf(format string, args ...any) {
	l.log(slog.LevelDebug, fmt.Sprintf(format, args...))
}
func (l pionLog) Error(msg string) { l.log(slog.LevelDebug, msg) }
func (l pionLog) Errorf(format string, args ...any) {
	l.log(slog.LevelDebug, fmt.Sprintf(format, args...))
}
