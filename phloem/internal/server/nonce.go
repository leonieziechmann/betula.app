package server

import (
	"bytes"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/binary"
	"net"
	"net/netip"

	"github.com/pion/stun/v4"
)

// nonceConn is TURN's UDP socket as Pion sees it. The source address of a datagram can be
// forged, and the address an allocation came from may be a peer of the space's other devices
// (permit), so Phloem takes nothing over UDP from an address that did not answer it: every
// nonce it hands out over UDP is bound to the address it goes to (Pion's nonce, a dot and an
// HMAC of the address and that nonce), and a request is taken only with the nonce of its own
// address. Such a request reaches Pion with Pion's nonce again and its MESSAGE-INTEGRITY made
// anew with the credential's key (Phloem holds it, and checks the device's first); a request
// with another nonce reaches it with one Pion does not know, which it answers 438 (stale nonce)
// with a fresh one, bound to the address it goes to. A device does what it always does, echo
// the nonce it got. TCP needs none of this: its handshake shows the address.
type nonceConn struct {
	net.PacketConn
	t *turnServer
}

func (c *nonceConn) WriteTo(b []byte, addr net.Addr) (int, error) {
	if isSTUN(b, classErrorResponse) {
		if out := c.t.bindNonce(b, addr); out != nil {
			if _, err := c.PacketConn.WriteTo(out, addr); err != nil {
				return 0, err
			}
			return len(b), nil
		}
	}
	return c.PacketConn.WriteTo(b, addr)
}

func (c *nonceConn) ReadFrom(b []byte) (int, net.Addr, error) {
	n, addr, err := c.PacketConn.ReadFrom(b)
	if err == nil && isSTUN(b[:n], classRequest) {
		n = c.t.checkNonce(b, n, addr)
	}
	return n, addr, err
}

// The class bits of a STUN message's type (RFC 8489 section 5).
const (
	classMask          = 0x0110
	classRequest       = 0x0000
	classErrorResponse = 0x0110
)

// isSTUN says whether b is a STUN message of the class, from its header alone (the relayed
// data, ChannelData and indications, is not looked into).
func isSTUN(b []byte, class uint16) bool {
	return stun.IsMessage(b) && b[0]&0xC0 == 0 && binary.BigEndian.Uint16(b[0:2])&classMask == class
}

// unknownNonce is a nonce Pion never made: it answers 438 with a fresh one.
var unknownNonce = []byte("0")

// nonceTag is the HMAC that binds nonce to addr's IP.
func (t *turnServer) nonceTag(addr net.Addr, nonce []byte) []byte {
	ip, _ := addrIP(addr)
	ip16 := ip.As16()
	mac := hmac.New(sha256.New, t.nonceKey)
	mac.Write(ip16[:])
	mac.Write(nonce)
	sum := mac.Sum(nil)[:12]
	return []byte(enc.EncodeToString(sum))
}

// bindNonce is Pion's error response with its nonce bound to addr; nil when it has none (or is
// signed, which Pion's challenges never are).
func (t *turnServer) bindNonce(b []byte, addr net.Addr) []byte {
	m := &stun.Message{Raw: append([]byte(nil), b...)}
	if m.Decode() != nil || !m.Contains(stun.AttrNonce) || m.Contains(stun.AttrMessageIntegrity) {
		return nil
	}
	nonce, _ := m.Get(stun.AttrNonce)
	bound := append(append(append([]byte(nil), nonce...), '.'), t.nonceTag(addr, nonce)...)
	return withNonce(m, bound, nil)
}

// checkNonce gives Pion back its nonce in a request with the nonce of its own address, signed
// anew with the credential's key where the device's signature holds; another nonce it replaces
// with one Pion does not know. It returns the length of the request in b now.
func (t *turnServer) checkNonce(b []byte, n int, addr net.Addr) int {
	m := &stun.Message{Raw: append([]byte(nil), b[:n]...)}
	if m.Decode() != nil || !m.Contains(stun.AttrNonce) {
		return n
	}
	value, _ := m.Get(stun.AttrNonce)
	var out []byte
	if i := bytes.LastIndexByte(value, '.'); i >= 0 && hmac.Equal(value[i+1:], t.nonceTag(addr, value[:i])) {
		nonce := value[:i]
		var key []byte
		if user, err := m.Get(stun.AttrUsername); err == nil && m.Contains(stun.AttrMessageIntegrity) {
			if k := t.key(string(user)); k != nil && stun.MessageIntegrity(k).Check(m) == nil {
				key = k
			}
		}
		out = withNonce(m, nonce, key)
	} else {
		t.s.m.turn.Inc("nonce")
		out = withNonce(m, unknownNonce, nil)
	}
	if len(out) > len(b) {
		return n // cannot be (both are shorter): Pion answers the nonce as it is with 438
	}
	return copy(b, out)
}

// withNonce is m with its NONCE replaced, its FINGERPRINT made anew; with key, its
// MESSAGE-INTEGRITY is made anew too, and what followed it is dropped (Pion checks
// MESSAGE-INTEGRITY alone), else every attribute stays as it was.
func withNonce(m *stun.Message, nonce, key []byte) []byte {
	out := &stun.Message{Type: m.Type, TransactionID: m.TransactionID}
	out.WriteHeader()
	fingerprint := false
attrs:
	for _, a := range m.Attributes {
		switch a.Type {
		case stun.AttrNonce:
			out.Add(stun.AttrNonce, nonce)
		case stun.AttrFingerprint:
			fingerprint = true
		case stun.AttrMessageIntegrity:
			if key != nil {
				_ = stun.MessageIntegrity(key).AddTo(out)
				for _, rest := range m.Attributes {
					if rest.Type == stun.AttrFingerprint {
						fingerprint = true
					}
				}
				break attrs
			}
			out.Add(a.Type, a.Value)
		default:
			out.Add(a.Type, a.Value)
		}
	}
	if fingerprint {
		_ = stun.Fingerprint.AddTo(out)
	}
	return out.Raw
}

// addrIP is the IP of a UDP or TCP address.
func addrIP(addr net.Addr) (netip.Addr, bool) {
	var ip net.IP
	switch a := addr.(type) {
	case *net.UDPAddr:
		ip = a.IP
	case *net.TCPAddr:
		ip = a.IP
	default:
		return netip.Addr{}, false
	}
	out, ok := netip.AddrFromSlice(ip)
	return out.Unmap(), ok
}
