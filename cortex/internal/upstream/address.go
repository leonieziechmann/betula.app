package upstream

import (
	"context"
	"fmt"
	"net"
	"net/netip"
	"syscall"
)

// blocked are the ranges no request may reach, whatever a name resolves to: everything that is
// not globally reachable unicast (IANA's special-purpose registries). Otherwise a client could
// make Cortex fetch Grafana, the Docker socket proxy or another service on the host network.
var blocked = func() []netip.Prefix {
	var out []netip.Prefix
	for _, s := range []string{
		"0.0.0.0/8",       // "this network", 0.0.0.0 among it
		"10.0.0.0/8",      // RFC 1918
		"100.64.0.0/10",   // carrier-grade NAT
		"127.0.0.0/8",     // loopback
		"169.254.0.0/16",  // link-local, the cloud metadata services among it
		"172.16.0.0/12",   // RFC 1918
		"192.0.0.0/24",    // IETF protocol assignments
		"192.0.2.0/24",    // documentation
		"192.88.99.0/24",  // the retired 6to4 relays
		"192.168.0.0/16",  // RFC 1918
		"198.18.0.0/15",   // benchmarking
		"198.51.100.0/24", // documentation
		"203.0.113.0/24",  // documentation
		"224.0.0.0/4",     // multicast
		"240.0.0.0/4",     // reserved, the broadcast address among it
		"::/96",           // unspecified, loopback and the deprecated IPv4-compatible form
		"64:ff9b:1::/48",  // NAT64 for local use
		"100::/64",        // discard
		"2001:db8::/32",   // documentation
		"fc00::/7",        // unique local
		"fe80::/10",       // link-local
		"fec0::/10",       // the deprecated site-local
		"ff00::/8",        // multicast
	} {
		out = append(out, netip.MustParsePrefix(s))
	}
	return out
}()

var (
	nat64     = netip.MustParsePrefix("64:ff9b::/96") // the IPv4 address in its last 32 bits
	sixToFour = netip.MustParsePrefix("2002::/16")    // the IPv4 address in bits 16 to 47
)

// publicAddr says whether ip may be fetched from. An IPv4 address in an IPv6 form (mapped,
// NAT64, 6to4) is judged by the IPv4 address it carries.
func publicAddr(ip netip.Addr) bool {
	ip = ip.WithZone("").Unmap()
	if !ip.IsValid() {
		return false
	}
	if ip.Is6() {
		b := ip.As16()
		switch {
		case nat64.Contains(ip):
			return publicAddr(netip.AddrFrom4([4]byte(b[12:16])))
		case sixToFour.Contains(ip):
			return publicAddr(netip.AddrFrom4([4]byte(b[2:6])))
		}
	}
	for _, p := range blocked {
		if p.Contains(ip) {
			return false
		}
	}
	return !(ip.IsLoopback() || ip.IsPrivate() || ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() ||
		ip.IsInterfaceLocalMulticast() || ip.IsMulticast() || ip.IsUnspecified())
}

func checkAddr(ip netip.Addr) error {
	if !publicAddr(ip) {
		return fmt.Errorf("%w: %s", ErrAddressNotAllowed, ip)
	}
	return nil
}

// refuseLiteral refuses a host that is an IP address no request may reach (unless
// AllowPrivate), before the request takes a slot of the host or reaches the transport: a
// client making up addresses would otherwise leave a host state, and a little memory of the
// transport for every address it failed to dial, behind each one. A name is left to the dialer.
func (u *Upstream) refuseLiteral(host string) error {
	if u.allowPrivate {
		return nil
	}
	ip, err := netip.ParseAddr(host)
	if err != nil {
		return nil
	}
	return checkAddr(ip)
}

// refusePrivate is the net.Dialer's Control: it runs after the name is resolved and before
// the connection is made, for every address the dialer tries, so a name that resolves to a
// private address (or is rebound to one) is refused all the same.
func refusePrivate(_, address string, _ syscall.RawConn) error {
	host, _, err := net.SplitHostPort(address)
	if err != nil {
		return fmt.Errorf("%w: %s", ErrAddressNotAllowed, address)
	}
	ip, err := netip.ParseAddr(host)
	if err != nil {
		return fmt.Errorf("%w: %s", ErrAddressNotAllowed, address)
	}
	return checkAddr(ip)
}

// guardedDial checks an address that is already an IP before the socket is made, then dials
// with refusePrivate as the Control: an IPv6 address on a host without IPv6 is refused as
// such, not as a network error that would count against the host.
func guardedDial(d *net.Dialer) func(ctx context.Context, network, address string) (net.Conn, error) {
	return func(ctx context.Context, network, address string) (net.Conn, error) {
		if host, _, err := net.SplitHostPort(address); err == nil {
			if ip, err := netip.ParseAddr(host); err == nil {
				if err := checkAddr(ip); err != nil {
					return nil, &net.OpError{Op: "dial", Net: network, Err: err}
				}
			}
		}
		return d.DialContext(ctx, network, address)
	}
}

// checkTarget resolves host and checks every address it has, for a request that goes through
// a proxy: the dialer then sees only the proxy's address, and the proxy resolves the name
// itself. A rebinding between this check and the proxy's lookup is not caught.
func checkTarget(ctx context.Context, host string) error {
	if ip, err := netip.ParseAddr(host); err == nil {
		return checkAddr(ip)
	}
	addrs, err := net.DefaultResolver.LookupNetIP(ctx, "ip", host)
	if err != nil {
		return err
	}
	if len(addrs) == 0 {
		return &net.DNSError{Err: "no addresses", Name: host, IsNotFound: true}
	}
	for _, ip := range addrs {
		if err := checkAddr(ip); err != nil {
			return fmt.Errorf("%s: %w", host, err)
		}
	}
	return nil
}
