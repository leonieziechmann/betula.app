package server

import (
	"encoding/json"
	"net"
	"net/http"
	"net/netip"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/coder/websocket"
	"github.com/pion/stun/v4"
	"github.com/pion/turn/v5"
)

// openFrom dials from the address named (as Traefik's X-Real-Ip says it) and says hello.
func (e *env) openFrom(addr string, p profile, device []byte) (*client, frame) {
	e.t.Helper()
	c := watch(e.t, e.mustDial(http.Header{"X-Real-Ip": {addr}}))
	c.send(hello(p, device, 0))
	f := c.next()
	if f.T != "welcome" {
		e.t.Fatalf("hello answered %s", f.raw)
	}
	return c, f
}

func TestASignalGoesToTheSlotNamedAlone(t *testing.T) {
	e := newEnv(t, nil)
	p, q := newProfile(), newProfile()
	da, db, dc := randBytes(deviceLen), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	e.mustCreate(q, dc)
	a, wa := e.connect(p, da, 0)
	b, wb := e.connect(p, db, 0)
	a.expect("presence")
	other, _ := e.connect(q, dc, 0)
	if !wa.Live || !wb.Live {
		t.Fatal("the welcome does not say that signal and turn are taken")
	}

	a.send(map[string]any{"t": "signal", "to": wb.Slot, "data": b64s([]byte("offer"))})
	f := b.expect("signal")
	if !strings.Contains(f.raw, `"from":`+strconv.Itoa(wa.Slot)) || decoded(t, f.Data) != "offer" {
		t.Fatalf("got %s", f.raw)
	}
	b.send(map[string]any{"t": "signal", "to": wa.Slot, "data": b64s([]byte("answer"))})
	if f := a.expect("signal"); decoded(t, f.Data) != "answer" {
		t.Fatalf("got %s", f.raw)
	}
	// A slot not online, or the sender's own: nobody hears it, the sender neither.
	a.send(map[string]any{"t": "signal", "to": 9, "data": b64s([]byte("x"))})
	a.send(map[string]any{"t": "signal", "to": wa.Slot, "data": b64s([]byte("x"))})
	a.quiet(200 * time.Millisecond)
	b.quiet(50 * time.Millisecond)
	other.quiet(50 * time.Millisecond)
}

func TestASignalAgainstTheProtocolClosesTheSocket(t *testing.T) {
	for name, f := range map[string]map[string]any{
		"no slot":   {"t": "signal", "data": b64s([]byte("x"))},
		"slot 0":    {"t": "signal", "to": 0, "data": b64s([]byte("x"))},
		"slot 17":   {"t": "signal", "to": 17, "data": b64s([]byte("x"))},
		"no data":   {"t": "signal", "to": 2},
		"too large": {"t": "signal", "to": 2, "data": b64s(make([]byte, 8<<10+1))},
	} {
		t.Run(name, func(t *testing.T) {
			e := newEnv(t, nil)
			p, d := newProfile(), randBytes(deviceLen)
			e.mustCreate(p, d)
			c, _ := e.connect(p, d, 0)
			c.send(f)
			if code, _ := c.closed(); code != websocket.StatusPolicyViolation {
				t.Fatalf("closed with %d", code)
			}
		})
	}
}

func TestSignalsAreHeldToTheirRate(t *testing.T) {
	e := newEnv(t, func(o *Options) { o.Limits.SignalRate, o.Limits.SignalBurst = 0.001, 2 })
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.connect(p, da, 0)
	b, wb := e.connect(p, db, 0)
	a.expect("presence")
	for range 3 {
		a.send(map[string]any{"t": "signal", "to": wb.Slot, "data": b64s([]byte("c"))})
	}
	b.expect("signal")
	b.expect("signal")
	if f := a.expect("error"); f.Code != "slow_down" {
		t.Fatalf("got %s", f.raw)
	}
	b.quiet(100 * time.Millisecond)
}

func TestWithoutTurnADeviceHearsSo(t *testing.T) {
	e := newEnv(t, nil)
	p, d := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, d)
	c, _ := e.connect(p, d, 0)
	c.send(map[string]string{"t": "turn"})
	if f := c.expect("turn"); f.raw != `{"t":"turn","urls":[]}` {
		t.Fatalf("got %s", f.raw)
	}
}

func TestOnlySpecialAddressesAreNotPublic(t *testing.T) {
	for addr, want := range map[string]bool{
		"1.1.1.1": true, "88.198.0.1": true, "11.0.0.1": true, "100.63.255.255": true, "100.128.0.1": true,
		"10.1.2.3": false, "172.16.0.1": false, "172.31.255.255": false, "172.30.1.1": false, "192.168.1.1": false,
		"127.0.0.1": false, "169.254.169.254": false, "100.64.0.1": false, "0.0.0.0": false, "224.0.0.251": false,
		"255.255.255.255": false, "192.0.2.1": false, "198.51.100.7": false, "203.0.113.9": false, "198.18.0.1": false,
		"2001:db8::1": false, "::ffff:10.0.0.1": false, "::ffff:1.1.1.1": true,
	} {
		if got := public(netip.MustParseAddr(addr)); got != want {
			t.Errorf("public(%s) = %v", addr, got)
		}
	}
}

// turnEnv is a Phloem with TURN on loopback: it announces its relays at 127.0.0.1 (its own
// address, never a peer) and lets the rest of 127.0.0.0/8 count as public.
func turnEnv(t *testing.T, mod func(*Options)) (*env, *TURN) {
	t.Helper()
	pc, err := net.ListenPacket("udp4", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	ln, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	tu := &TURN{
		Host: "betula.test", Port: pc.LocalAddr().(*net.UDPAddr).Port, IP: netip.MustParseAddr("127.0.0.1"),
		RelayMin: 41000, RelayMax: 41063, TLSHost: "turn.betula.test", PacketConn: pc, Listener: ln,
		RelayBind: "127.0.0.1", loopback: true,
	}
	e := newEnv(t, func(o *Options) {
		o.TURN = tu
		if mod != nil {
			mod(o)
		}
	})
	return e, tu
}

// credential asks Phloem for a socket's TURN credential.
func credential(t *testing.T, c *client) (urls []string, user, password string) {
	t.Helper()
	c.send(map[string]string{"t": "turn"})
	var f struct {
		T        string   `json:"t"`
		URLs     []string `json:"urls"`
		User     string   `json:"user"`
		Password string   `json:"password"`
	}
	raw := c.expect("turn").raw
	if err := json.Unmarshal([]byte(raw), &f); err != nil || f.User == "" || f.Password == "" {
		t.Fatalf("turn answered %s", raw)
	}
	return f.URLs, f.User, f.Password
}

// turnClient is a TURN client on a socket of its own, with one allocation.
func turnClient(t *testing.T, tu *TURN, user, password string) (*turn.Client, net.PacketConn, error) {
	t.Helper()
	return turnClientFrom(t, tu, "127.0.0.1:0", user, password)
}

// turnClientFrom is turnClient on a socket bound to bind: its allocation comes from there.
func turnClientFrom(t *testing.T, tu *TURN, bind, user, password string) (*turn.Client, net.PacketConn, error) {
	t.Helper()
	conn, err := net.ListenPacket("udp4", bind)
	if err != nil {
		t.Fatal(err)
	}
	server := tu.PacketConn.LocalAddr().String()
	cl, err := turn.NewClient(&turn.ClientConfig{
		STUNServerAddr: server, TURNServerAddr: server, Conn: conn,
		Username: user, Password: password, Realm: tu.Host, LoggerFactory: pionLogs{},
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		cl.Close()
		_ = conn.Close()
	})
	if err := cl.Listen(); err != nil {
		t.Fatal(err)
	}
	relay, err := cl.Allocate()
	if relay != nil {
		t.Cleanup(func() { _ = relay.Close() })
	}
	return cl, relay, err
}

func TestTurnRelaysToTheOtherDevicesOfTheSpaceAlone(t *testing.T) {
	e, tu := turnEnv(t, nil)
	p, q := newProfile(), newProfile()
	da, db, dc, dd := randBytes(deviceLen), randBytes(deviceLen), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	e.mustCreate(q, dc)
	a, _ := e.openFrom("127.0.0.10", p, da)
	b, _ := e.openFrom("127.0.0.20", p, db)
	a.expect("presence")
	e.openFrom("127.0.0.30", q, dc)       // another space's device
	d, _ := e.openFrom("10.1.2.3", p, dd) // a device whose address is no peer, whatever it says
	a.expect("presence")
	b.expect("presence")

	urls, user, password := credential(t, a)
	want := "stun:betula.test:" + strconv.Itoa(tu.Port) + " turn:betula.test:" + strconv.Itoa(tu.Port) + "?transport=udp turn:betula.test:" +
		strconv.Itoa(tu.Port) + "?transport=tcp turns:turn.betula.test:443?transport=tcp"
	if strings.Join(urls, " ") != want {
		t.Fatalf("urls %v", urls)
	}
	if _, u2, _ := credential(t, a); u2 != user {
		t.Fatal("a socket asking again got another credential")
	}
	cl, relay, err := turnClient(t, tu, user, password)
	if err != nil {
		t.Fatalf("allocate: %v", err)
	}
	if ra := relay.LocalAddr().(*net.UDPAddr); !ra.IP.Equal(net.IPv4(127, 0, 0, 1)) || ra.Port < 41000 || ra.Port > 41063 {
		t.Fatalf("relay at %s", ra)
	}

	// b's address: a peer. A datagram goes there, and one from there comes back.
	peer, err := net.ListenPacket("udp4", "127.0.0.20:0")
	if err != nil {
		t.Fatal(err)
	}
	defer peer.Close()
	if err := cl.CreatePermission(peer.LocalAddr()); err != nil {
		t.Fatalf("permission for the space's other device: %v", err)
	}
	if _, err := relay.WriteTo([]byte("to b"), peer.LocalAddr()); err != nil {
		t.Fatal(err)
	}
	buf := make([]byte, 64)
	_ = peer.SetReadDeadline(time.Now().Add(3 * time.Second))
	n, from, err := peer.ReadFrom(buf)
	if err != nil || string(buf[:n]) != "to b" || from.String() != relay.LocalAddr().String() {
		t.Fatalf("the peer got %q from %v (%v)", buf[:n], from, err)
	}
	if _, err := peer.WriteTo([]byte("to a"), relay.LocalAddr()); err != nil {
		t.Fatal(err)
	}
	_ = relay.SetReadDeadline(time.Now().Add(3 * time.Second))
	n, _, err = relay.ReadFrom(buf)
	if err != nil || string(buf[:n]) != "to a" {
		t.Fatalf("the relay got %q (%v)", buf[:n], err)
	}

	// Everything else is refused: another space's device, no device at all, a private address
	// a socket came from, Phloem's own address.
	for _, addr := range []string{"127.0.0.30:5000", "127.0.0.40:5000", "10.1.2.3:5000", "127.0.0.1:5000", "192.168.1.1:5000"} {
		if err := cl.CreatePermission(udpAddr(addr)); err == nil {
			t.Errorf("a permission for %s", addr)
		}
	}
	// a's own address is no peer of its own: only another socket's counts.
	if err := cl.CreatePermission(udpAddr("127.0.0.10:5000")); err == nil {
		t.Error("a permission for the asking socket's own address")
	}

	// Once b is gone, so is its address.
	b.send(map[string]string{"t": "leave"})
	b.closed()
	a.expect("presence")
	eventually(t, "b's address refused", func() bool { return cl.CreatePermission(udpAddr("127.0.0.20:6000")) != nil })
	_ = d
}

func TestATurnCredentialEndsWithItsSocket(t *testing.T) {
	e, tu := turnEnv(t, nil)
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.openFrom("127.0.0.10", p, da)
	_, user, password := credential(t, a)
	if _, _, err := turnClient(t, tu, user, password); err != nil {
		t.Fatalf("allocate: %v", err)
	}
	if _, _, err := turnClient(t, tu, user, "wrong"); err == nil {
		t.Fatal("an allocation with a wrong password")
	}
	a.send(map[string]string{"t": "leave"})
	a.closed()
	eventually(t, "the credential refused", func() bool {
		_, _, err := turnClient(t, tu, user, password)
		return err != nil
	})
}

func TestTurnAllocationsAreBounded(t *testing.T) {
	e, tu := turnEnv(t, func(o *Options) { o.Limits.AllocsPerSocket = 2 })
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.openFrom("127.0.0.10", p, da)
	_, user, password := credential(t, a)
	for i := range 2 {
		if _, _, err := turnClient(t, tu, user, password); err != nil {
			t.Fatalf("allocation %d: %v", i+1, err)
		}
	}
	if _, _, err := turnClient(t, tu, user, password); err == nil {
		t.Fatal("a third allocation of one socket")
	}
	// No TCP relays (RFC 6062): WebRTC needs none. Asked over TURN's TCP, as RFC 6062 has it.
	tcp, err := net.Dial("tcp4", tu.Listener.Addr().String())
	if err != nil {
		t.Fatal(err)
	}
	defer tcp.Close()
	b, _ := e.openFrom("127.0.0.11", p, randBytes(deviceLen))
	_, u2, pw2 := credential(t, b)
	cl, err := turn.NewClient(&turn.ClientConfig{TURNServerAddr: tu.Listener.Addr().String(), Conn: turn.NewSTUNConn(tcp),
		Username: u2, Password: pw2, Realm: tu.Host, LoggerFactory: pionLogs{}})
	if err != nil {
		t.Fatal(err)
	}
	defer cl.Close()
	if err := cl.Listen(); err != nil {
		t.Fatal(err)
	}
	if _, err := cl.AllocateTCP(); err == nil {
		t.Fatal("a TCP relay")
	}
}

func TestARelayKeepsToItsRate(t *testing.T) {
	e, tu := turnEnv(t, func(o *Options) { o.Limits.RelayRate = 2000 })
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.openFrom("127.0.0.10", p, da)
	e.openFrom("127.0.0.20", p, db)
	a.expect("presence")
	_, user, password := credential(t, a)
	cl, relay, err := turnClient(t, tu, user, password)
	if err != nil {
		t.Fatal(err)
	}
	peer, err := net.ListenPacket("udp4", "127.0.0.20:0")
	if err != nil {
		t.Fatal(err)
	}
	defer peer.Close()
	if err := cl.CreatePermission(peer.LocalAddr()); err != nil {
		t.Fatal(err)
	}
	for range 10 { // 10 KB at once, the bucket holds 2 KB
		if _, err := relay.WriteTo(make([]byte, 1000), peer.LocalAddr()); err != nil {
			t.Fatal(err)
		}
	}
	got := 0
	buf := make([]byte, 2000)
	for {
		_ = peer.SetReadDeadline(time.Now().Add(300 * time.Millisecond))
		n, _, err := peer.ReadFrom(buf)
		if err != nil {
			break
		}
		got += n
	}
	if got < 1000 || got > 3000 {
		t.Fatalf("the peer got %d bytes of 10000 at 2000 a second", got)
	}
}

func TestTurnNeedsAPublicAddress(t *testing.T) {
	pc, _ := net.ListenPacket("udp4", "127.0.0.1:0")
	ln, _ := net.Listen("tcp4", "127.0.0.1:0")
	defer pc.Close()
	defer ln.Close()
	st := newEnv(t, nil).st
	_, err := New(Options{Store: st, TURN: &TURN{Host: "x", Port: 3478, IP: netip.MustParseAddr("10.0.0.1"),
		RelayMin: 49160, RelayMax: 49161, PacketConn: pc, Listener: ln}})
	if err == nil {
		t.Fatal("TURN at a private address")
	}
}

func udpAddr(s string) *net.UDPAddr {
	a, _ := net.ResolveUDPAddr("udp4", s)
	return a
}

// A device whose socket comes from one address and its UDP from another (a VPN, or a socket
// over IPv6 and UDP over IPv4) is a peer at the address its allocation came from, for the other
// devices of its space alone, while its socket lives.
func TestTurnRelaysToTheAddressADeviceAllocatedFrom(t *testing.T) {
	e, tu := turnEnv(t, nil)
	p, q := newProfile(), newProfile()
	da, db, dc := randBytes(deviceLen), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	e.mustCreate(q, dc)
	a, _ := e.openFrom("127.0.0.10", p, da)
	b, _ := e.openFrom("127.0.0.20", p, db)
	a.expect("presence")
	c, _ := e.openFrom("127.0.0.30", q, dc) // another space's device
	_, ua, pa := credential(t, a)
	_, ub, pb := credential(t, b)
	_, uc, pc := credential(t, c)

	cl, relay, err := turnClientFrom(t, tu, "127.0.0.11:0", ua, pa)
	if err != nil {
		t.Fatalf("a's allocation: %v", err)
	}
	// Before b allocated, its UDP address is nobody's.
	if err := cl.CreatePermission(udpAddr("127.0.0.21:5000")); err == nil {
		t.Fatal("a permission for an address no allocation came from yet")
	}
	if _, _, err := turnClientFrom(t, tu, "127.0.0.21:0", ub, pb); err != nil {
		t.Fatalf("b's allocation: %v", err)
	}
	if _, _, err := turnClientFrom(t, tu, "127.0.0.31:0", uc, pc); err != nil {
		t.Fatalf("c's allocation: %v", err)
	}

	// A moment later, b's UDP address is a peer: a datagram goes there.
	peer, err := net.ListenPacket("udp4", "127.0.0.21:0")
	if err != nil {
		t.Fatal(err)
	}
	defer peer.Close()
	if err := cl.CreatePermission(peer.LocalAddr()); err != nil {
		t.Fatalf("a permission for the address b allocated from: %v", err)
	}
	if _, err := relay.WriteTo([]byte("to b"), peer.LocalAddr()); err != nil {
		t.Fatal(err)
	}
	buf := make([]byte, 64)
	_ = peer.SetReadDeadline(time.Now().Add(3 * time.Second))
	if n, _, err := peer.ReadFrom(buf); err != nil || string(buf[:n]) != "to b" {
		t.Fatalf("the peer got %q (%v)", buf[:n], err)
	}
	// Not another space's allocation, nor a's own, nor an address nobody allocated from.
	for _, addr := range []string{"127.0.0.31:5000", "127.0.0.11:5000", "127.0.0.22:5000"} {
		if err := cl.CreatePermission(udpAddr(addr)); err == nil {
			t.Errorf("a permission for %s", addr)
		}
	}
	if !e.metric(`phloem_turn_total{event="learned"} 3`) {
		t.Error("the addresses learned are not counted")
	}

	// Once b's socket ended, the address is nobody's again.
	b.send(map[string]string{"t": "leave"})
	b.closed()
	a.expect("presence")
	eventually(t, "b's allocation address refused", func() bool { return cl.CreatePermission(udpAddr("127.0.0.21:6000")) != nil })
}

// A poll session's socket carries the device's address: a peer as any socket's.
func TestTurnSeesTheAddressOfAPollSession(t *testing.T) {
	e, tu := turnEnv(t, nil)
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.openFrom("127.0.0.10", p, da)
	status, id, _ := e.openPoll(map[string]string{"X-Real-Ip": "127.0.0.20"}, hello(p, db, 0))
	if status != http.StatusCreated {
		t.Fatal(status)
	}
	b := e.poller(id, map[string]string{"X-Real-Ip": "127.0.0.20"})
	b.start()
	b.expect("welcome")
	a.expect("presence")
	_, user, password := credential(t, a)
	cl, _, err := turnClient(t, tu, user, password)
	if err != nil {
		t.Fatal(err)
	}
	if err := cl.CreatePermission(udpAddr("127.0.0.20:5000")); err != nil {
		t.Fatalf("a permission for the poll session's address: %v", err)
	}
	// Its own credential, asked for over HTTPS, allocates as a socket's does.
	b.send(map[string]string{"t": "turn"})
	var f struct{ User, Password string }
	_ = json.Unmarshal([]byte(b.expect("turn").raw), &f)
	if _, _, err := turnClientFrom(t, tu, "127.0.0.21:0", f.User, f.Password); err != nil {
		t.Fatalf("the poll session's allocation: %v", err)
	}
	if err := cl.CreatePermission(udpAddr("127.0.0.21:5000")); err != nil {
		t.Fatalf("a permission for the poll session's UDP address: %v", err)
	}
}

// An allocation's address counts only where it may be a peer at all: never a special address
// (TURN over TLS comes from Traefik's), never Phloem's own.
func TestTurnLearnsNoSpecialAddress(t *testing.T) {
	e, _ := turnEnv(t, nil)
	p, da, db := newProfile(), randBytes(deviceLen), randBytes(deviceLen)
	e.mustCreate(p, da)
	e.openFrom("127.0.0.10", p, da)
	b, _ := e.openFrom("127.0.0.20", p, db)
	_, ub, _ := credential(t, b)
	ts := e.srv.turn
	for _, src := range []net.Addr{
		udpAddr("10.0.0.5:4000"), udpAddr("127.0.0.1:4000"), udpAddr("192.168.1.9:4000"), udpAddr("127.0.0.20:4000"),
		&net.TCPAddr{IP: net.IPv4(10, 0, 1, 7), Port: 51000}, // Traefik, for TURN over TLS
	} {
		ts.allocated(src, nil, "UDP", ub, tu(e), nil, 0)
		ts.released(src, nil, "UDP", ub, tu(e))
	}
	ts.mu.Lock()
	from := ts.users[ub].from
	ts.mu.Unlock()
	if len(from) != 0 {
		t.Fatalf("learned %v", from)
	}
	if e.metric(`phloem_turn_total{event="learned"}`) {
		t.Error("learned counted")
	}
}

func tu(e *env) string { return e.srv.turn.realm }

// Over UDP an allocation needs the nonce Phloem handed to its own address: a datagram that
// claims another address cannot make Phloem take that address for a device's.
func TestTurnNoncesAreBoundToTheirAddress(t *testing.T) {
	e, tun := turnEnv(t, nil)
	p, da := newProfile(), randBytes(deviceLen)
	e.mustCreate(p, da)
	a, _ := e.openFrom("127.0.0.10", p, da)
	_, user, password := credential(t, a)
	key := turn.GenerateAuthKey(user, tun.Host, password)
	server := tun.PacketConn.LocalAddr()
	listen := func(addr string) net.PacketConn {
		c, err := net.ListenPacket("udp4", addr)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = c.Close() })
		return c
	}
	ask := func(c net.PacketConn, nonce []byte) *stun.Message {
		t.Helper()
		m := stun.MustBuild(stun.TransactionID, stun.NewType(stun.MethodAllocate, stun.ClassRequest))
		m.Add(stun.AttrRequestedTransport, []byte{17, 0, 0, 0})
		if nonce != nil {
			m.Add(stun.AttrUsername, []byte(user))
			m.Add(stun.AttrRealm, []byte(tun.Host))
			m.Add(stun.AttrNonce, nonce)
			if err := stun.MessageIntegrity(key).AddTo(m); err != nil {
				t.Fatal(err)
			}
		}
		if err := stun.Fingerprint.AddTo(m); err != nil {
			t.Fatal(err)
		}
		if _, err := c.WriteTo(m.Raw, server); err != nil {
			t.Fatal(err)
		}
		buf := make([]byte, 1500)
		_ = c.SetReadDeadline(time.Now().Add(3 * time.Second))
		n, _, err := c.ReadFrom(buf)
		if err != nil {
			t.Fatalf("no answer: %v", err)
		}
		resp := &stun.Message{Raw: buf[:n]}
		if err := resp.Decode(); err != nil {
			t.Fatal(err)
		}
		return resp
	}
	code := func(m *stun.Message) stun.ErrorCode {
		var ec stun.ErrorCodeAttribute
		if m.Type.Class == stun.ClassSuccessResponse {
			return 0
		}
		_ = ec.GetFrom(m)
		return ec.Code
	}
	nonceOf := func(m *stun.Message) []byte {
		v, err := m.Get(stun.AttrNonce)
		if err != nil {
			t.Fatalf("no nonce: %v", err)
		}
		return append([]byte(nil), v...)
	}

	x, y, z := listen("127.0.0.41:0"), listen("127.0.0.42:0"), listen("127.0.0.43:0")
	first := ask(x, nil)
	if code(first) != stun.CodeUnauthorized {
		t.Fatalf("x without a credential: %d", code(first))
	}
	nx := nonceOf(first)
	if !strings.Contains(string(nx), ".") {
		t.Fatalf("x's nonce %q is not bound", nx)
	}
	if got := ask(y, nx); code(got) != stun.CodeStaleNonce || !strings.Contains(string(nonceOf(got)), ".") {
		t.Fatalf("y with x's nonce: %d", code(got))
	}
	nz := nonceOf(ask(z, nil))
	pions := nz[:strings.LastIndexByte(string(nz), '.')]
	if got := ask(z, pions); code(got) != stun.CodeStaleNonce {
		t.Fatalf("z with Pion's own nonce: %d", code(got))
	}
	if got := ask(x, nx); code(got) != 0 {
		t.Fatalf("x with its own nonce: %d", code(got))
	}
	if !e.metric(`phloem_turn_total{event="nonce"} 2`) {
		t.Error("the foreign nonces are not counted")
	}
}
